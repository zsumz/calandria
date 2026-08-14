use core::fmt;

use calandria::{Moment, Retained, RetainedBytes, Span, Turn, WorkCount};
use calandria_sim::{
    ActionContext, Delivery, DutyId, EventToken, Model, Simulation, SimulationLimits,
    SimulationPhase, StepError, TimelineId, Topology,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Cancel(EventToken),
    Future,
    Inserted,
}

impl Retained for Event {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::ZERO
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ModelFailure;

impl fmt::Display for ModelFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("modeled failure")
    }
}

impl core::error::Error for ModelFailure {}

#[derive(Debug)]
struct FailingCancellation;

impl Model for FailingCancellation {
    type Event = Event;
    type Observation = ();
    type Error = ModelFailure;

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
        duty: DutyId,
        delivery: Delivery<Self::Event>,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        let Event::Cancel(token) = delivery.into_event() else {
            return Ok(Turn::waiting());
        };
        context
            .cancel(token)
            .unwrap_or_else(|error| panic!("future event must be cancellable: {error}"));
        let _ = context
            .send_after(duty, Span::from_nanos(5), Event::Inserted)
            .unwrap_or_else(|error| panic!("replacement must fit: {error}"));
        Err(ModelFailure)
    }
}

#[test]
fn failed_action_rolls_back_send_and_cancellation_together() {
    let topology = one_duty();
    let mut simulation = Simulation::new(
        TimelineId::new(31),
        FailingCancellation,
        topology,
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));
    let future = simulation
        .inject_at(DutyId::new(0), Moment::from_nanos(10), Event::Future)
        .unwrap_or_else(|error| panic!("future event must fit: {error}"));
    let _ = simulation
        .inject(DutyId::new(0), Event::Cancel(future))
        .unwrap_or_else(|error| panic!("command must fit: {error}"));

    let error = match simulation.step() {
        Ok(_) => panic!("simulation step must fail"),
        Err(error) => error,
    };
    assert!(matches!(error, StepError::Model { .. }));
    assert_eq!(simulation.phase(), SimulationPhase::Failed);
    assert_eq!(simulation.snapshot().pending_events(), 1);
    assert_eq!(
        simulation.snapshot().next_event_at(),
        Some(Moment::from_nanos(10))
    );
}

#[derive(Debug)]
struct SuccessfulCancellation;

impl Model for SuccessfulCancellation {
    type Event = Event;
    type Observation = ();
    type Error = ModelFailure;

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
        delivery: Delivery<Self::Event>,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        let Event::Cancel(token) = delivery.into_event() else {
            return Ok(Turn::waiting());
        };
        context
            .cancel(token)
            .unwrap_or_else(|error| panic!("future event must be cancellable: {error}"));
        Ok(Turn::stopped(WorkCount::new(1)))
    }
}

#[test]
fn successful_action_commits_exact_cancellation() {
    let mut simulation = Simulation::new(
        TimelineId::new(32),
        SuccessfulCancellation,
        one_duty(),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));
    let future = simulation
        .inject_at(DutyId::new(0), Moment::from_nanos(10), Event::Future)
        .unwrap_or_else(|error| panic!("future event must fit: {error}"));
    let _ = simulation
        .inject(DutyId::new(0), Event::Cancel(future))
        .unwrap_or_else(|error| panic!("command must fit: {error}"));

    let report = simulation
        .run_to_completion()
        .unwrap_or_else(|error| panic!("world must complete: {error}"));
    assert_eq!(report.snapshot().pending_events(), 0);
}

#[derive(Debug)]
struct StopsWithSelfDelivery;

impl Model for StopsWithSelfDelivery {
    type Event = Event;
    type Observation = ();
    type Error = ModelFailure;

    fn turn(
        &mut self,
        duty: DutyId,
        _now: Moment,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        let _ = context
            .send(duty, Event::Inserted)
            .unwrap_or_else(|error| panic!("self delivery must fit: {error}"));
        Ok(Turn::stopped(WorkCount::new(1)))
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
fn stop_with_pending_owned_work_fails_closed_and_rolls_back() {
    let mut simulation = Simulation::new(
        TimelineId::new(33),
        StopsWithSelfDelivery,
        one_duty(),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));

    let error = match simulation.step() {
        Ok(_) => panic!("simulation step must fail"),
        Err(error) => error,
    };
    assert!(matches!(error, StepError::Kernel(_)));
    assert_eq!(simulation.phase(), SimulationPhase::Failed);
    assert_eq!(simulation.snapshot().pending_events(), 0);
}

fn one_duty() -> Topology {
    Topology::new([DutyId::new(0)])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}
