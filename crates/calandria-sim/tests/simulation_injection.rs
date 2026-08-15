//! External simulation injection rejection and ownership tests.

use core::{fmt, num::NonZeroUsize};
use std::panic::{AssertUnwindSafe, catch_unwind};

use calandria::{Moment, Retained, RetainedBytes, Span, Turn, WorkCount};
use calandria_sim::{
    ActionContext, Delivery, DutyId, Fifo, InjectionError, InjectionFailure, Model, NoopMonitor,
    ScheduleFailure, Simulation, SimulationLimits, SimulationPhase, StepError, TimelineId,
    TimelineLimits, Topology,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnedEvent {
    id: u8,
    bytes: RetainedBytes,
}

impl Retained for OwnedEvent {
    fn retained_bytes(&self) -> RetainedBytes {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Waiting,
    StopOne,
    StopAll,
    Fail,
    Panic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlannedFailure;

impl fmt::Display for PlannedFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("planned injection-world failure")
    }
}

impl core::error::Error for PlannedFailure {}

#[derive(Debug)]
struct World {
    mode: Mode,
}

impl Model for World {
    type Event = OwnedEvent;
    type Observation = ();
    type Error = PlannedFailure;

    fn turn(
        &mut self,
        duty: DutyId,
        _now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        match self.mode {
            Mode::StopOne if duty == DutyId::new(1) => Ok(Turn::stopped(WorkCount::new(1))),
            Mode::Waiting | Mode::StopOne => Ok(Turn::waiting()),
            Mode::StopAll => Ok(Turn::stopped(WorkCount::new(1))),
            Mode::Fail => Err(PlannedFailure),
            Mode::Panic => panic!("planned simulation poison"),
        }
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
fn target_and_time_rejections_return_the_exact_event() {
    let mut ordinary = simulation(Mode::Waiting, topology([1]), SimulationLimits::default());
    let unknown = ordinary
        .inject(DutyId::new(9), event(1, 0))
        .unwrap_err_or_else(|| panic!("unknown duty must reject"));
    assert_eq!(
        unknown.failure(),
        InjectionFailure::UnknownTarget(DutyId::new(9))
    );
    assert_eq!(unknown.into_event(), event(1, 0));

    let limits = SimulationLimits::default().with_max_virtual_time(Moment::from_nanos(5));
    let mut bounded = simulation(Mode::Waiting, topology([1]), limits);
    let failure = rejected(
        bounded
            .inject_at(DutyId::new(1), Moment::from_nanos(6), event(2, 0))
            .unwrap_err_or_else(|| panic!("event above time ceiling must reject")),
        event(2, 0),
    );
    assert_eq!(
        failure,
        InjectionFailure::BeyondTimeLimit {
            requested: Moment::from_nanos(6),
            limit: Moment::from_nanos(5),
        }
    );

    let mut final_moment = simulation_at(
        Mode::Waiting,
        topology([1]),
        SimulationLimits::default(),
        Moment::from_nanos(u64::MAX),
    );
    let failure = rejected(
        final_moment
            .inject_after(DutyId::new(1), Span::from_nanos(1), event(3, 0))
            .unwrap_err_or_else(|| panic!("relative time must not wrap")),
        event(3, 0),
    );
    assert!(matches!(failure, InjectionFailure::TimeOverflow { .. }));
}

#[test]
fn timeline_rejections_remain_exact_at_the_simulation_boundary() {
    let count_limits =
        SimulationLimits::new(TimelineLimits::new(nonzero(1), RetainedBytes::new(8)));
    let mut count = simulation(Mode::Waiting, topology([1]), count_limits);
    count
        .inject(DutyId::new(1), event(1, 1))
        .unwrap_or_else(|error| panic!("first event must fit: {error}"));
    let failure = rejected(
        count
            .inject(DutyId::new(1), event(2, 1))
            .unwrap_err_or_else(|| panic!("second event must exceed count")),
        event(2, 1),
    );
    assert!(matches!(
        failure,
        InjectionFailure::Timeline(ScheduleFailure::EventCapacity { .. })
    ));

    let byte_limits = SimulationLimits::new(TimelineLimits::new(nonzero(2), RetainedBytes::new(4)));
    let mut bytes = simulation(Mode::Waiting, topology([1]), byte_limits);
    bytes
        .inject(DutyId::new(1), event(3, 3))
        .unwrap_or_else(|error| panic!("first retained event must fit: {error}"));
    let failure = rejected(
        bytes
            .inject(DutyId::new(1), event(4, 2))
            .unwrap_err_or_else(|| panic!("second event must exceed retained bytes")),
        event(4, 2),
    );
    assert!(matches!(
        failure,
        InjectionFailure::Timeline(ScheduleFailure::RetainedByteCapacity { .. })
    ));

    let mut past = simulation_at(
        Mode::Waiting,
        topology([1]),
        SimulationLimits::default(),
        Moment::from_nanos(10),
    );
    let failure = rejected(
        past.inject_at(DutyId::new(1), Moment::from_nanos(9), event(5, 0))
            .unwrap_err_or_else(|| panic!("past injection must reject")),
        event(5, 0),
    );
    assert!(matches!(
        failure,
        InjectionFailure::Timeline(ScheduleFailure::ScheduledInPast { .. })
    ));
}

#[test]
fn lifecycle_rejections_distinguish_stopped_inactive_and_poisoned_worlds() {
    let mut stopped = simulation(Mode::StopOne, topology([1, 2]), SimulationLimits::default());
    stopped
        .step()
        .unwrap_or_else(|error| panic!("first duty must stop: {error}"));
    assert_eq!(stopped.phase(), SimulationPhase::Active);
    let failure = rejected(
        stopped
            .inject(DutyId::new(1), event(1, 0))
            .unwrap_err_or_else(|| panic!("stopped duty must reject")),
        event(1, 0),
    );
    assert_eq!(failure, InjectionFailure::TargetStopped(DutyId::new(1)));

    let mut completed = simulation(Mode::StopAll, topology([1]), SimulationLimits::default());
    completed
        .run_to_completion()
        .unwrap_or_else(|error| panic!("world must complete: {error}"));
    let failure = rejected(
        completed
            .inject(DutyId::new(1), event(2, 0))
            .unwrap_err_or_else(|| panic!("completed world must reject")),
        event(2, 0),
    );
    assert_eq!(
        failure,
        InjectionFailure::Inactive(SimulationPhase::Completed)
    );

    let mut failed = simulation(Mode::Fail, topology([1]), SimulationLimits::default());
    let error = failed
        .step()
        .unwrap_err_or_else(|| panic!("planned model failure must reject the step"));
    assert_eq!(
        error.to_string(),
        "model action 0 failed: planned injection-world failure"
    );
    assert!(matches!(error, StepError::Model { .. }));
    let failure = rejected(
        failed
            .inject(DutyId::new(1), event(3, 0))
            .unwrap_err_or_else(|| panic!("failed world must reject")),
        event(3, 0),
    );
    assert_eq!(failure, InjectionFailure::Inactive(SimulationPhase::Failed));

    let mut poisoned = simulation(Mode::Panic, topology([1]), SimulationLimits::default());
    assert!(catch_unwind(AssertUnwindSafe(|| poisoned.step())).is_err());
    assert!(poisoned.is_poisoned());
    let failure = rejected(
        poisoned
            .inject(DutyId::new(1), event(4, 0))
            .unwrap_err_or_else(|| panic!("poisoned world must reject")),
        event(4, 0),
    );
    assert_eq!(failure, InjectionFailure::Poisoned);
}

trait ResultExt<T, E> {
    fn unwrap_err_or_else(self, on_ok: impl FnOnce() -> E) -> E;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn unwrap_err_or_else(self, on_ok: impl FnOnce() -> E) -> E {
        match self {
            Ok(_) => on_ok(),
            Err(error) => error,
        }
    }
}

fn rejected(error: InjectionError<OwnedEvent>, expected: OwnedEvent) -> InjectionFailure {
    assert!(!error.to_string().is_empty());
    let (event, failure) = error.into_parts();
    assert_eq!(event, expected);
    failure
}

fn simulation(mode: Mode, topology: Topology, limits: SimulationLimits) -> Simulation<World> {
    simulation_at(mode, topology, limits, Moment::ORIGIN)
}

fn simulation_at(
    mode: Mode,
    topology: Topology,
    limits: SimulationLimits,
    now: Moment,
) -> Simulation<World> {
    Simulation::with_parts(
        TimelineId::new(82),
        now,
        World { mode },
        topology,
        limits,
        Fifo::new(),
        NoopMonitor::new(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"))
}

fn topology<const N: usize>(duties: [u32; N]) -> Topology {
    Topology::new(duties.map(DutyId::new))
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}

const fn event(id: u8, bytes: u64) -> OwnedEvent {
    OwnedEvent {
        id,
        bytes: RetainedBytes::new(bytes),
    }
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
