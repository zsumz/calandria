//! Simulation construction and successful component-recovery tests.

use core::convert::Infallible;

use calandria::{Moment, Turn};
use calandria_sim::{
    ActionContext, ActionKey, ActionRecord, Delivery, DutyId, Model, Monitor, ReadySet, Scheduler,
    Simulation, SimulationBuildFailure, SimulationLimits, SimulationView, TimelineId, Topology,
};

#[derive(Debug, Eq, PartialEq)]
struct OwnedModel(&'static str);

impl Model for OwnedModel {
    type Event = ();
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        _duty: DutyId,
        _now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        Ok(Turn::waiting())
    }

    fn deliver(
        &mut self,
        _duty: DutyId,
        _delivery: Delivery<Self::Event>,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        Ok(Turn::waiting())
    }
}

#[derive(Debug, Eq, PartialEq)]
struct OwnedScheduler(&'static str);

impl Scheduler for OwnedScheduler {
    type Error = Infallible;

    fn choose(&mut self, _now: Moment, ready: ReadySet<'_>) -> Result<ActionKey, Self::Error> {
        Ok(ready
            .first()
            .unwrap_or_else(|| panic!("scheduler requires one ready action")))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct OwnedMonitor(&'static str);

impl Monitor<OwnedModel> for OwnedMonitor {
    type Error = Infallible;

    fn after_action(
        &mut self,
        _view: SimulationView<'_, OwnedModel>,
        _action: &ActionRecord<()>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn build_rejection_returns_every_consumer_component() {
    let limits = SimulationLimits::default().with_max_duties(nonzero(1));
    let Err(error) = Simulation::with_parts(
        TimelineId::new(1),
        Moment::ORIGIN,
        OwnedModel("model"),
        topology([1, 2]),
        limits,
        OwnedScheduler("scheduler"),
        OwnedMonitor("monitor"),
    ) else {
        panic!("oversized topology must reject construction");
    };

    assert!(matches!(
        error.failure(),
        SimulationBuildFailure::DutyCapacity { actual: 2, .. }
    ));
    let (failure, model, topology, scheduler, monitor) = error.into_parts();
    assert!(matches!(
        failure,
        SimulationBuildFailure::DutyCapacity { .. }
    ));
    assert_eq!(model, OwnedModel("model"));
    assert_eq!(topology.duties(), [DutyId::new(1), DutyId::new(2)]);
    assert_eq!(scheduler, OwnedScheduler("scheduler"));
    assert_eq!(monitor, OwnedMonitor("monitor"));
}

#[test]
fn successful_simulation_returns_every_consumer_component() {
    let simulation = Simulation::with_parts(
        TimelineId::new(2),
        Moment::ORIGIN,
        OwnedModel("model"),
        topology([1]),
        SimulationLimits::default(),
        OwnedScheduler("scheduler"),
        OwnedMonitor("monitor"),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));

    let (model, topology, scheduler, monitor) = simulation.into_components();
    assert_eq!(model, OwnedModel("model"));
    assert_eq!(topology.duties(), [DutyId::new(1)]);
    assert_eq!(scheduler, OwnedScheduler("scheduler"));
    assert_eq!(monitor, OwnedMonitor("monitor"));
}

fn topology<const N: usize>(duties: [u32; N]) -> Topology {
    Topology::new(duties.map(DutyId::new))
        .unwrap_or_else(|error| panic!("test topology must be valid: {error}"))
}

fn nonzero(value: usize) -> core::num::NonZeroUsize {
    core::num::NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
