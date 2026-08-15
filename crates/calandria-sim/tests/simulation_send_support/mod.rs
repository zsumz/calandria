//! Minimal model fixture for transactional send-boundary tests.

use core::num::NonZeroUsize;

use calandria::{Moment, Retained, RetainedBytes, Span, Turn, WorkCount};
use calandria_sim::{
    ActionContext, Delivery, DutyId, Fifo, Model, NoopMonitor, SendError, SendFailure, Simulation,
    SimulationLimits, TimelineId, Topology,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Event {
    id: u8,
    bytes: RetainedBytes,
}

impl Retained for Event {
    fn retained_bytes(&self) -> RetainedBytes {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Scenario {
    EffectCapacity,
    EffectByteCapacity,
    EffectByteOverflow,
    UnknownTarget,
    TargetStopped,
    TimeOverflow,
    BeyondTime,
    TimelineCount,
    TimelineBytes,
    TimelineOverflow,
    TimelinePast,
}

#[derive(Debug)]
pub(crate) struct World {
    scenario: Scenario,
    pub(crate) rejected: Option<(Event, SendFailure)>,
}

impl World {
    const fn new(scenario: Scenario) -> Self {
        Self {
            scenario,
            rejected: None,
        }
    }

    fn capture(&mut self, result: Result<calandria_sim::EventToken, SendError<Event>>) {
        let Err(error) = result else {
            panic!("modeled send must reject");
        };
        assert!(!error.to_string().is_empty());
        self.rejected = Some(error.into_parts());
    }

    fn capture_event(&mut self, result: Result<calandria_sim::EventToken, SendError<Event>>) {
        let Err(error) = result else {
            panic!("modeled send must reject");
        };
        let failure = error.failure();
        let event = error.into_event();
        self.rejected = Some((event, failure));
    }
}

impl Model for World {
    type Event = Event;
    type Observation = ();
    type Error = core::convert::Infallible;

    fn turn(
        &mut self,
        duty: DutyId,
        _now: Moment,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        match self.scenario {
            Scenario::EffectCapacity => {
                send(context, duty, event(1, 0));
                self.capture(context.send(duty, event(2, 0)));
            }
            Scenario::EffectByteCapacity => {
                send(context, duty, event(1, 2));
                self.capture(context.send(duty, event(2, 2)));
            }
            Scenario::EffectByteOverflow => {
                send(context, duty, event(1, 1));
                self.capture(context.send(duty, event(2, u64::MAX)));
            }
            Scenario::UnknownTarget => {
                self.capture_event(context.send(DutyId::new(99), event(3, 0)));
            }
            Scenario::TargetStopped if duty == DutyId::new(1) => {
                return Ok(Turn::stopped(WorkCount::new(1)));
            }
            Scenario::TargetStopped => {
                self.capture(context.send(DutyId::new(1), event(4, 0)));
            }
            Scenario::TimeOverflow => {
                self.capture(context.send_after(duty, Span::from_nanos(1), event(5, 0)));
            }
            Scenario::BeyondTime => {
                self.capture(context.send_at(duty, Moment::from_nanos(6), event(6, 0)));
            }
            Scenario::TimelineCount => {
                self.capture(context.send(duty, event(7, 0)));
            }
            Scenario::TimelineBytes => {
                self.capture(context.send(duty, event(8, 2)));
            }
            Scenario::TimelineOverflow => {
                self.capture(context.send(duty, event(9, u64::MAX)));
            }
            Scenario::TimelinePast => {
                self.capture(context.send_at(duty, Moment::from_nanos(9), event(10, 0)));
            }
        }
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

pub(crate) fn assert_send_failure(
    scenario: Scenario,
    limits: SimulationLimits,
    now: Moment,
    topology: Topology,
    event: Event,
    failure: SendFailure,
) {
    let mut simulation = simulation(scenario, limits, now, topology);
    step(&mut simulation);
    assert_eq!(simulation.model().rejected, Some((event, failure)));
}

pub(crate) fn simulation(
    scenario: Scenario,
    limits: SimulationLimits,
    now: Moment,
    topology: Topology,
) -> Simulation<World> {
    Simulation::with_parts(
        TimelineId::new(91),
        now,
        World::new(scenario),
        topology,
        limits,
        Fifo::new(),
        NoopMonitor::new(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"))
}

pub(crate) fn step(simulation: &mut Simulation<World>) {
    simulation
        .step()
        .unwrap_or_else(|error| panic!("modeled action must run: {error}"));
}

pub(crate) fn inject_future(simulation: &mut Simulation<World>, event: Event) {
    simulation
        .inject_at(DutyId::new(1), Moment::from_nanos(1), event)
        .unwrap_or_else(|error| panic!("setup event must fit: {error}"));
}

pub(crate) fn topology<const N: usize>(duties: [u32; N]) -> Topology {
    Topology::new(duties.map(DutyId::new))
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}

pub(crate) const fn event(id: u8, bytes: u64) -> Event {
    Event {
        id,
        bytes: RetainedBytes::new(bytes),
    }
}

pub(crate) fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}

fn send(context: &mut ActionContext<'_, Event, ()>, target: DutyId, event: Event) {
    context
        .send(target, event)
        .unwrap_or_else(|error| panic!("first modeled send must fit: {error}"));
}
