//! Deterministic simulation failure and terminal-state tests.

use core::{
    convert::Infallible,
    fmt,
    num::{NonZeroU64, NonZeroUsize},
};

use calandria::{Deadline, Moment, RetainedBytes, Turn, WorkCount};
use calandria_sim::{
    ActionContext, ActionKey, ActionRecord, Delivery, DutyId, Fifo, LimitFailure, Model, Monitor,
    NoopMonitor, ReadySet, Scheduler, Simulation, SimulationBuildError, SimulationLimits,
    SimulationPhase, SimulationView, StepError, TimelineId, TimelineLimits, Topology,
};

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
        Ok(Turn::runnable(WorkCount::new(1)))
    }
}

#[test]
fn zero_time_livelock_is_bounded_explicitly() {
    let limits = SimulationLimits::default().with_actions_per_moment(
        NonZeroU64::new(3).unwrap_or_else(|| panic!("test limit must be nonzero")),
    );
    let mut simulation = Simulation::new(TimelineId::new(41), AlwaysRunnable, one_duty(), limits)
        .unwrap_or_else(|error| panic!("simulation must build: {error}"));

    for _ in 0..3 {
        let _ = simulation
            .step()
            .unwrap_or_else(|error| panic!("bounded action must run: {error}"));
    }
    let Err(error) = simulation.step() else {
        panic!("simulation step must fail");
    };
    assert!(matches!(
        error,
        StepError::Limit(LimitFailure::ActionsAtMoment { .. })
    ));
    assert_eq!(simulation.phase(), SimulationPhase::Failed);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MonitorFailure;

impl fmt::Display for MonitorFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invariant failed")
    }
}

impl core::error::Error for MonitorFailure {}

#[derive(Debug)]
struct RejectFirst;

impl Monitor<AlwaysRunnable> for RejectFirst {
    type Error = MonitorFailure;

    fn after_action(
        &mut self,
        view: SimulationView<'_, AlwaysRunnable>,
        action: &ActionRecord<()>,
    ) -> Result<(), Self::Error> {
        assert_eq!(view.snapshot().actions(), 1);
        assert_eq!(action.meta().id().get(), 0);
        Err(MonitorFailure)
    }
}

#[test]
fn monitor_observes_committed_state_then_fails_terminally() {
    let simulation = Simulation::new(
        TimelineId::new(42),
        AlwaysRunnable,
        one_duty(),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));
    let mut simulation = simulation.with_monitor(RejectFirst);

    let Err(error) = simulation.step() else {
        panic!("simulation step must fail");
    };
    assert!(matches!(error, StepError::Monitor { .. }));
    assert_eq!(simulation.phase(), SimulationPhase::Failed);
    assert_eq!(simulation.snapshot().actions(), 1);
}

#[derive(Debug)]
struct InvalidScheduler;

impl Scheduler for InvalidScheduler {
    type Error = Infallible;

    fn choose(&mut self, _now: Moment, _ready: ReadySet<'_>) -> Result<ActionKey, Self::Error> {
        Ok(ActionKey::turn(DutyId::new(999)))
    }
}

#[test]
fn scheduler_cannot_invent_an_action() {
    let mut simulation = Simulation::with_scheduler(
        TimelineId::new(43),
        AlwaysRunnable,
        one_duty(),
        SimulationLimits::default(),
        InvalidScheduler,
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));

    let Err(error) = simulation.step() else {
        panic!("simulation step must fail");
    };
    assert!(matches!(error, StepError::InvalidSelection(_)));
    assert_eq!(simulation.phase(), SimulationPhase::Failed);
    assert_eq!(simulation.snapshot().actions(), 0);
}

#[derive(Debug)]
struct FutureDeadline;

impl Model for FutureDeadline {
    type Event = ();
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        _duty: DutyId,
        _now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        Ok(Turn::until(
            WorkCount::ZERO,
            Deadline::at(Moment::from_nanos(10)),
        ))
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
fn virtual_time_limit_fails_terminally() {
    let limits = SimulationLimits::default().with_max_virtual_time(Moment::from_nanos(5));
    let mut simulation = Simulation::new(TimelineId::new(44), FutureDeadline, one_duty(), limits)
        .unwrap_or_else(|error| panic!("simulation must build: {error}"));
    let _ = simulation
        .step()
        .unwrap_or_else(|error| panic!("initial owner turn must run: {error}"));

    let Err(error) = simulation.step() else {
        panic!("simulation step must fail");
    };
    assert!(matches!(
        error,
        StepError::Limit(LimitFailure::VirtualTime { .. })
    ));
    assert_eq!(simulation.phase(), SimulationPhase::Failed);
}

#[test]
fn initial_time_cannot_begin_beyond_the_ceiling() {
    let limits = SimulationLimits::default().with_max_virtual_time(Moment::from_nanos(5));
    let Err(error) = Simulation::with_parts(
        TimelineId::new(45),
        Moment::from_nanos(6),
        AlwaysRunnable,
        one_duty(),
        limits,
        Fifo::new(),
        NoopMonitor::new(),
    ) else {
        panic!("simulation construction must fail");
    };
    assert!(matches!(
        error,
        SimulationBuildError::InitialTimeBeyondLimit { .. }
    ));
}

#[test]
fn ready_capacity_overflow_is_rejected_before_allocation() {
    let event_capacity =
        NonZeroUsize::new(usize::MAX).unwrap_or_else(|| panic!("maximum usize must be nonzero"));
    let limits = SimulationLimits::new(TimelineLimits::new(event_capacity, RetainedBytes::ZERO));
    let Err(error) = Simulation::new(TimelineId::new(46), AlwaysRunnable, one_duty(), limits)
    else {
        panic!("combined ready capacity must overflow");
    };
    assert!(matches!(
        error,
        SimulationBuildError::ReadyCapacityOverflow { .. }
    ));
}

fn one_duty() -> Topology {
    Topology::new([DutyId::new(0)])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}
