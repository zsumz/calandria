//! Bounded causal trace retention and exact replay divergence tests.

use core::{convert::Infallible, num::NonZeroUsize};

use calandria::{Moment, Retained, RetainedBytes, Turn, WorkCount};
use calandria_sim::{
    ActionContext, ActionKind, Delivery, DutyId, EntropySeed, EntropyStreamId, Model,
    ReplayDivergence, RunEnd, Seeded, Simulation, SimulationLimits, StepError, TimelineId,
    Topology, Trace, TraceError, TraceLimits,
};

#[derive(Clone, Copy, Debug)]
enum Message {
    Ping(u8),
    Stop,
}

impl Retained for Message {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::ZERO
    }
}

#[derive(Debug, Default)]
struct CausalWorld {
    started: bool,
    deliveries: u64,
}

impl Model for CausalWorld {
    type Event = Message;
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        duty: DutyId,
        _now: Moment,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        if duty == DutyId::new(1) && !self.started {
            self.started = true;
            send(context, DutyId::new(2), Message::Ping(1));
        }
        Ok(Turn::waiting())
    }

    fn deliver(
        &mut self,
        duty: DutyId,
        delivery: Delivery<Self::Event>,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        self.deliveries = self.deliveries.saturating_add(1);
        let other = if duty == DutyId::new(1) {
            DutyId::new(2)
        } else {
            DutyId::new(1)
        };
        Ok(match delivery.into_event() {
            Message::Ping(1) => {
                send(context, other, Message::Ping(0));
                Turn::waiting()
            }
            Message::Ping(0) => {
                send(context, other, Message::Stop);
                Turn::stopped(WorkCount::new(1))
            }
            Message::Ping(_) => Turn::waiting(),
            Message::Stop => Turn::stopped(WorkCount::new(1)),
        })
    }
}

#[test]
fn bounded_causal_trace_replays_exact_committed_actions() {
    let scheduler = Seeded::new(EntropySeed::new(47), EntropyStreamId::new(3));
    let mut original = Simulation::with_scheduler(
        TimelineId::new(61),
        CausalWorld::default(),
        topology(),
        SimulationLimits::default(),
        scheduler,
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"))
    .with_monitor(Trace::new(trace_limits(16)));
    let report = original
        .run_to_completion()
        .unwrap_or_else(|error| panic!("traced world must complete: {error}"));
    assert_eq!(report.end(), RunEnd::Completed);

    let trace = original.monitor();
    assert_eq!(trace.snapshot().entries(), 5);
    assert!(trace.entries().any(|entry| {
        matches!(entry.meta().key().kind(), ActionKind::Delivery(_))
            && entry.meta().cause().is_some()
    }));
    let replay = trace.replay();
    let mut repeated = Simulation::with_scheduler(
        TimelineId::new(61),
        CausalWorld::default(),
        topology(),
        SimulationLimits::default(),
        replay,
    )
    .unwrap_or_else(|error| panic!("replay must build: {error}"));
    let report = repeated
        .run_to_completion()
        .unwrap_or_else(|error| panic!("exact replay must complete: {error}"));

    assert_eq!(report.end(), RunEnd::Completed);
    assert!(repeated.scheduler().is_complete());
    assert_eq!(repeated.model().deliveries, original.model().deliveries);
}

#[derive(Debug)]
struct OneTurn {
    work: WorkCount,
}

impl Model for OneTurn {
    type Event = ();
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        _duty: DutyId,
        _now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        Ok(Turn::stopped(self.work))
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
fn replay_reports_committed_turn_divergence_at_the_exact_position() {
    let original = OneTurn {
        work: WorkCount::new(1),
    };
    let mut traced = Simulation::new(
        TimelineId::new(62),
        original,
        one_duty(),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"))
    .with_monitor(Trace::new(trace_limits(2)));
    traced
        .run_to_completion()
        .unwrap_or_else(|error| panic!("trace source must complete: {error}"));

    let replay = traced.monitor().replay();
    let mut changed = Simulation::with_scheduler(
        TimelineId::new(62),
        OneTurn {
            work: WorkCount::new(2),
        },
        one_duty(),
        SimulationLimits::default(),
        replay,
    )
    .unwrap_or_else(|error| panic!("replay must build: {error}"));
    let Err(error) = changed.step() else {
        panic!("changed turn must diverge");
    };

    assert!(matches!(
        error,
        StepError::Scheduler(ReplayDivergence::Committed { position, .. })
            if position.get() == 0
    ));
}

#[derive(Debug)]
struct AlwaysRunnable;

impl Model for AlwaysRunnable {
    type Event = ();
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        _duty: DutyId,
        _now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        Ok(Turn::runnable(WorkCount::new(1)))
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
fn trace_capacity_is_a_terminal_post_commit_failure() {
    let mut simulation = Simulation::new(
        TimelineId::new(63),
        AlwaysRunnable,
        one_duty(),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"))
    .with_monitor(Trace::new(trace_limits(1)));
    simulation
        .step()
        .unwrap_or_else(|error| panic!("first trace entry must fit: {error}"));
    let Err(error) = simulation.step() else {
        panic!("second trace entry must exceed capacity");
    };

    assert!(matches!(
        error,
        StepError::Monitor {
            source: TraceError::Capacity { .. },
            ..
        }
    ));
    assert_eq!(simulation.monitor().snapshot().entries(), 1);
}

fn send(context: &mut ActionContext<'_, Message, ()>, target: DutyId, message: Message) {
    let _ = context
        .send(target, message)
        .unwrap_or_else(|error| panic!("causal message must fit: {error}"));
}

fn trace_limits(entries: usize) -> TraceLimits {
    TraceLimits::new(nonzero(entries), RetainedBytes::ZERO)
}

fn topology() -> Topology {
    Topology::new([DutyId::new(1), DutyId::new(2)])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}

fn one_duty() -> Topology {
    Topology::new([DutyId::new(1)])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
