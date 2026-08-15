//! Transactional modeled-cancellation rejection and rollback tests.

use core::num::NonZeroUsize;

use calandria::{Moment, Retained, RetainedBytes, Span, Turn, WorkCount};
use calandria_sim::{
    ActionContext, CancelFailure, Delivery, DutyId, EventToken, Model, Simulation,
    SimulationLimits, Timeline, TimelineId, TimelineLimits, Topology,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Command(Command),
    Payload { id: u8, bytes: RetainedBytes },
}

impl Retained for Event {
    fn retained_bytes(&self) -> RetainedBytes {
        match self {
            Self::Command(_) => RetainedBytes::ZERO,
            Self::Payload { bytes, .. } => *bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    CancelInserted,
    EffectCapacity,
    Foreign(EventToken),
    NotPending,
    Pair(EventToken, EventToken),
}

#[derive(Debug, Default)]
struct World {
    rejected: Option<CancelFailure>,
}

impl World {
    fn capture(&mut self, result: Result<(), CancelFailure>) {
        let failure = match result {
            Ok(()) => panic!("modeled cancellation must reject"),
            Err(failure) => failure,
        };
        assert!(!failure.to_string().is_empty());
        self.rejected = Some(failure);
    }
}

impl Model for World {
    type Event = Event;
    type Observation = ();
    type Error = core::convert::Infallible;

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
        let Event::Command(command) = delivery.into_event() else {
            return Ok(Turn::waiting());
        };
        match command {
            Command::CancelInserted => {
                let token = send(context, duty, payload(1, 2));
                cancel(context, token);
                return Ok(Turn::stopped(WorkCount::new(1)));
            }
            Command::EffectCapacity => {
                let token = send(context, duty, payload(2, 0));
                self.capture(context.cancel(token));
            }
            Command::Foreign(token) => self.capture(context.cancel(token)),
            Command::NotPending => {
                let token = send(context, duty, payload(3, 0));
                cancel(context, token);
                self.capture(context.cancel(token));
            }
            Command::Pair(first, second) => {
                cancel(context, first);
                self.capture(context.cancel(second));
            }
        }
        Ok(Turn::waiting())
    }
}

#[test]
fn canceling_an_inserted_effect_removes_it_before_commit() {
    let mut simulation = simulation(SimulationLimits::default());
    inject_command(&mut simulation, Command::CancelInserted);

    let report = simulation
        .run_to_completion()
        .unwrap_or_else(|error| panic!("inserted cancellation must complete: {error}"));
    assert_eq!(report.snapshot().pending_events(), 0);
    assert_eq!(simulation.model().rejected, None);
}

#[test]
fn cancellation_rejects_effect_capacity_foreign_and_stale_tokens() {
    let limits = SimulationLimits::default().with_effects_per_action(nonzero(1));
    let mut capacity = simulation(limits);
    inject_command(&mut capacity, Command::EffectCapacity);
    step(&mut capacity);
    assert_eq!(
        capacity.model().rejected,
        Some(CancelFailure::EffectCapacity { limit: nonzero(1) })
    );
    assert_eq!(capacity.snapshot().pending_events(), 1);

    let foreign_token = foreign_token();
    let mut foreign = simulation(SimulationLimits::default());
    inject_command(&mut foreign, Command::Foreign(foreign_token));
    step(&mut foreign);
    assert_eq!(
        foreign.model().rejected,
        Some(CancelFailure::ForeignTimeline {
            expected: TimelineId::new(92),
            actual: TimelineId::new(99),
        })
    );

    let mut stale = simulation(SimulationLimits::default());
    inject_command(&mut stale, Command::NotPending);
    step(&mut stale);
    assert!(matches!(
        stale.model().rejected,
        Some(CancelFailure::NotPending(token))
            if token.timeline() == TimelineId::new(92)
                && token.at() == Moment::from_nanos(1)
    ));
    assert_eq!(stale.snapshot().pending_events(), 0);
}

#[test]
fn canceled_rollback_ownership_is_retained_within_the_action_byte_limit() {
    let timeline = TimelineLimits::new(nonzero(4), RetainedBytes::new(4));
    let limits =
        SimulationLimits::new(timeline).with_effect_bytes_per_action(RetainedBytes::new(3));
    let mut simulation = simulation(limits);
    let first = inject_future(&mut simulation, payload(4, 2), 10);
    let second = inject_future(&mut simulation, payload(5, 2), 11);
    inject_command(&mut simulation, Command::Pair(first, second));

    step(&mut simulation);
    assert_eq!(
        simulation.model().rejected,
        Some(CancelFailure::RetainedByteCapacity {
            limit: RetainedBytes::new(3),
            current: RetainedBytes::new(2),
            event: RetainedBytes::new(2),
        })
    );
    assert_eq!(simulation.snapshot().pending_events(), 1);
    assert_eq!(simulation.snapshot().next_event_at(), Some(second.at()));

    let overflow = CancelFailure::RetainedByteOverflow {
        current: RetainedBytes::new(u64::MAX),
        event: RetainedBytes::new(1),
    };
    assert_eq!(
        overflow.to_string(),
        "retaining 1 canceled bytes beside 18446744073709551615 would overflow"
    );
}

fn simulation(limits: SimulationLimits) -> Simulation<World> {
    Simulation::new(TimelineId::new(92), World::default(), topology(), limits)
        .unwrap_or_else(|error| panic!("simulation must build: {error}"))
}

fn step(simulation: &mut Simulation<World>) {
    simulation
        .step()
        .unwrap_or_else(|error| panic!("modeled action must run: {error}"));
}

fn inject_command(simulation: &mut Simulation<World>, command: Command) {
    simulation
        .inject(DutyId::new(1), Event::Command(command))
        .unwrap_or_else(|error| panic!("command must fit: {error}"));
}

fn inject_future(simulation: &mut Simulation<World>, event: Event, at: u64) -> EventToken {
    simulation
        .inject_at(DutyId::new(1), Moment::from_nanos(at), event)
        .unwrap_or_else(|error| panic!("future event must fit: {error}"))
}

fn foreign_token() -> EventToken {
    let limits = TimelineLimits::new(nonzero(1), RetainedBytes::ZERO);
    let mut timeline = Timeline::new(TimelineId::new(99), limits);
    timeline
        .schedule_at(Moment::ORIGIN, payload(9, 0))
        .unwrap_or_else(|error| panic!("foreign token setup must fit: {error}"))
}

fn send(context: &mut ActionContext<'_, Event, ()>, duty: DutyId, event: Event) -> EventToken {
    context
        .send_after(duty, Span::from_nanos(1), event)
        .unwrap_or_else(|error| panic!("inserted event must fit: {error}"))
}

fn cancel(context: &mut ActionContext<'_, Event, ()>, token: EventToken) {
    context
        .cancel(token)
        .unwrap_or_else(|error| panic!("pending event must cancel: {error}"));
}

fn topology() -> Topology {
    Topology::new([DutyId::new(1)])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}

const fn payload(id: u8, bytes: u64) -> Event {
    Event::Payload {
        id,
        bytes: RetainedBytes::new(bytes),
    }
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
