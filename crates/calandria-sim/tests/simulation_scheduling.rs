//! Ready-set ordering and scheduler-selection tests.

use core::convert::Infallible;

use calandria::{Moment, Retained, RetainedBytes, Turn, WorkCount};
use calandria_sim::{
    ActionContext, Delivery, DutyId, Fifo, Model, RoundRobin, Simulation, SimulationLimits, Step,
    TimelineId, Topology,
};

#[derive(Clone, Copy, Debug)]
struct Event;

impl Retained for Event {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::ZERO
    }
}

#[derive(Debug, Default)]
struct Recorder {
    turns: Vec<DutyId>,
    deliveries: Vec<DutyId>,
}

impl Model for Recorder {
    type Event = Event;
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        duty: DutyId,
        _now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        self.turns.push(duty);
        Ok(Turn::runnable(WorkCount::new(1)))
    }

    fn deliver(
        &mut self,
        duty: DutyId,
        _delivery: Delivery<Self::Event>,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        self.deliveries.push(duty);
        Ok(Turn::waiting())
    }
}

#[test]
fn fifo_preserves_equal_time_event_admission_across_owners() {
    let mut simulation = Simulation::with_scheduler(
        TimelineId::new(51),
        Recorder::default(),
        topology([1, 2]),
        SimulationLimits::default(),
        Fifo::new(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));
    let _ = simulation
        .inject(DutyId::new(2), Event)
        .unwrap_or_else(|error| panic!("first event must fit: {error}"));
    let _ = simulation
        .inject(DutyId::new(1), Event)
        .unwrap_or_else(|error| panic!("second event must fit: {error}"));

    assert!(matches!(simulation.step(), Ok(Step::Action(_))));
    assert!(matches!(simulation.step(), Ok(Step::Action(_))));
    assert_eq!(
        simulation.model().deliveries.as_slice(),
        &[DutyId::new(2), DutyId::new(1)]
    );
}

#[test]
fn round_robin_rotates_owners_independently_of_action_order() {
    let mut simulation = Simulation::with_scheduler(
        TimelineId::new(52),
        Recorder::default(),
        topology([1, 2, 3]),
        SimulationLimits::default(),
        RoundRobin::new(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));

    for _ in 0..5 {
        let _ = simulation
            .step()
            .unwrap_or_else(|error| panic!("owner turn must run: {error}"));
    }
    assert_eq!(
        simulation.model().turns.as_slice(),
        &[
            DutyId::new(1),
            DutyId::new(2),
            DutyId::new(3),
            DutyId::new(1),
            DutyId::new(2),
        ]
    );
}

fn topology<const N: usize>(duties: [u32; N]) -> Topology {
    Topology::new(duties.map(DutyId::new))
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}
