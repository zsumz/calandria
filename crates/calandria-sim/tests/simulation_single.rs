//! Single-duty deterministic lifecycle tests.

use core::convert::Infallible;

use calandria::{Deadline, Duty, Moment, Retained, RetainedBytes, Turn, WorkCount};
use calandria_sim::{
    ActionContext, Delivery, DutyId, Model, RunEnd, Simulation, SimulationLimits, TimelineId,
    Topology,
};

#[derive(Debug)]
struct Maintenance {
    turns: u8,
}

impl Duty for Maintenance {
    type Error = Infallible;

    fn turn(&mut self, now: Moment) -> Result<Turn, Self::Error> {
        self.turns = self.turns.saturating_add(1);
        if now < Moment::from_nanos(10) {
            Ok(Turn::until(
                WorkCount::new(1),
                Deadline::at(Moment::from_nanos(10)),
            ))
        } else {
            Ok(Turn::stopped(WorkCount::new(1)))
        }
    }
}

#[derive(Debug)]
struct MaintenanceModel {
    duty: Maintenance,
}

impl Model for MaintenanceModel {
    type Event = ();
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        duty: DutyId,
        now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        assert_eq!(duty, DutyId::new(7));
        self.duty.turn(now)
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

#[test]
fn production_duty_runs_to_completion_under_virtual_time() {
    let topology = Topology::new([DutyId::new(7)])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"));
    let model = MaintenanceModel {
        duty: Maintenance { turns: 0 },
    };
    let mut simulation = Simulation::new(
        TimelineId::new(11),
        model,
        topology,
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));

    let report = simulation
        .run_to_completion()
        .unwrap_or_else(|error| panic!("simulation must complete: {error}"));

    assert_eq!(report.end(), RunEnd::Completed);
    assert_eq!(report.snapshot().now(), Moment::from_nanos(10));
    assert_eq!(simulation.model().duty.turns, 2);
    assert_eq!(report.snapshot().actions(), 2);
}

#[derive(Debug)]
struct WaitingModel;

impl Model for WaitingModel {
    type Event = Wake;
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
        Ok(Turn::stopped(WorkCount::new(1)))
    }
}

#[derive(Debug)]
struct Wake;

impl Retained for Wake {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::ZERO
    }
}

#[test]
fn quiescent_world_can_receive_work_and_resume() {
    let topology = Topology::new([DutyId::new(0)])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"));
    let mut simulation = Simulation::new(
        TimelineId::new(12),
        WaitingModel,
        topology,
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));

    let quiescent = simulation
        .run_to_quiescence()
        .unwrap_or_else(|error| panic!("simulation must quiesce: {error}"));
    assert_eq!(quiescent.end(), RunEnd::Quiescent);

    let _ = simulation
        .inject(DutyId::new(0), Wake)
        .unwrap_or_else(|error| panic!("active quiescent simulation accepts work: {error}"));
    let completed = simulation
        .run_to_completion()
        .unwrap_or_else(|error| panic!("injected world must complete: {error}"));
    assert_eq!(completed.end(), RunEnd::Completed);
}
