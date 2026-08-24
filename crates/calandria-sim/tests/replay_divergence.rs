//! Replay selection divergence and composed trace-monitor tests.

use core::{convert::Infallible, fmt, num::NonZeroUsize};

use calandria::{Moment, RetainedBytes, Turn, WorkCount};
use calandria_sim::{
    ActionContext, ActionKey, ActionRecord, Delivery, DutyId, Model, Monitor, NoopMonitor,
    ReadySet, Replay, ReplayDivergence, Scheduler, Simulation, SimulationLimits, SimulationPhase,
    SimulationView, StepError, TimelineId, Topology, Trace, TraceEntry, TraceError, TraceLimits,
};

#[derive(Debug)]
struct StoppingWorld;

impl Model for StoppingWorld {
    type Event = ();
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        _duty: DutyId,
        _now: Moment,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        assert_eq!(context.action().get(), 0);
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
fn replay_reports_moment_and_unavailable_action_before_model_execution() {
    let (replay, _) = recorded_action();
    let mut late = replay_world(Moment::from_nanos(1), topology([1]), replay);
    let Err(error) = late.step() else {
        panic!("changed virtual moment must diverge");
    };
    assert_eq!(
        error.to_string(),
        "scheduler failed: position 0 expected 0ns but reached 1ns"
    );
    assert!(matches!(
        error,
        StepError::Scheduler(ReplayDivergence::Moment {
            position,
            actual,
            ..
        }) if position.get() == 0 && actual == Moment::from_nanos(1)
    ));
    assert_eq!(late.snapshot().actions(), 0);

    let (replay, _) = recorded_action();
    let mut different_owner = replay_world(Moment::ORIGIN, topology([2]), replay);
    let Err(error) = different_owner.step() else {
        panic!("missing traced owner must diverge");
    };
    assert_eq!(
        error.to_string(),
        "scheduler failed: position 0 expected duty 1 absent from 1 ready actions"
    );
    assert!(matches!(
        error,
        StepError::Scheduler(ReplayDivergence::Unavailable {
            position,
            ready: 1,
            ..
        }) if position.get() == 0
    ));
    assert_eq!(different_owner.snapshot().actions(), 0);
}

#[test]
fn replay_rejects_commit_without_selection_and_double_selection() {
    let (mut replay, entry) = recorded_action();
    let error = replay
        .committed(entry.meta(), entry.turn())
        .unwrap_err_or_else(|| panic!("unselected commit must diverge"));
    assert_eq!(
        error.to_string(),
        "position 0 received an unselected commit"
    );
    assert_eq!(error.position().get(), 0);
    assert!(matches!(
        error,
        ReplayDivergence::UnexpectedCommit { position, actual }
            if position.get() == 0 && actual == entry
    ));

    let (replay, _) = recorded_action();
    let mut simulation = Simulation::with_scheduler(
        TimelineId::new(83),
        StoppingWorld,
        topology([1]),
        SimulationLimits::default(),
        ChooseTwice { replay },
    )
    .unwrap_or_else(|error| panic!("double-selection replay must build: {error}"));
    let Err(error) = simulation.step() else {
        panic!("second selection without commit must diverge");
    };
    assert_eq!(
        error.to_string(),
        "scheduler failed: position 0 still awaits its commit"
    );
    assert!(matches!(
        error,
        StepError::Scheduler(ReplayDivergence::PendingCommit { position })
            if position.get() == 0
    ));
    assert_eq!(simulation.phase(), SimulationPhase::Failed);
    assert_eq!(simulation.snapshot().actions(), 0);
}

#[derive(Debug)]
struct ChooseTwice {
    replay: Replay,
}

impl Scheduler for ChooseTwice {
    type Error = ReplayDivergence;

    fn choose(&mut self, now: Moment, ready: ReadySet<'_>) -> Result<ActionKey, Self::Error> {
        let _ = self.replay.choose(now, ready)?;
        self.replay.choose(now, ready)
    }
}

#[test]
fn replay_rejects_completion_while_a_selection_awaits_commit() {
    let (replay, _) = recorded_action();
    let mut simulation = Simulation::with_scheduler(
        TimelineId::new(83),
        StoppingWorld,
        topology([1]),
        SimulationLimits::default(),
        DropCommits(replay),
    )
    .unwrap_or_else(|error| panic!("pending-commit replay must build: {error}"));
    assert!(simulation.step().is_ok());
    assert_eq!(simulation.scheduler().0.position().get(), 0);
    let Err(error) = simulation.step() else {
        panic!("completion with an uncommitted selection must diverge");
    };
    assert!(matches!(
        error,
        StepError::Scheduler(ReplayDivergence::PendingCommit { .. })
    ));
}

struct DropCommits(Replay);

impl Scheduler for DropCommits {
    type Error = ReplayDivergence;

    fn choose(&mut self, now: Moment, ready: ReadySet<'_>) -> Result<ActionKey, Self::Error> {
        self.0.choose(now, ready)
    }

    fn finished(&mut self) -> Result<(), Self::Error> {
        self.0.finished()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MonitorFailure;

impl fmt::Display for MonitorFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("planned composed monitor failure")
    }
}

impl core::error::Error for MonitorFailure {}

#[derive(Debug, Default)]
struct RejectMonitor {
    calls: u64,
}

impl Monitor<StoppingWorld> for RejectMonitor {
    type Error = MonitorFailure;

    fn after_action(
        &mut self,
        _view: SimulationView<'_, StoppingWorld>,
        _action: &ActionRecord<()>,
    ) -> Result<(), Self::Error> {
        self.calls += 1;
        Err(MonitorFailure)
    }
}

#[test]
fn trace_records_before_its_composed_monitor_fails() {
    let limits = trace_limits(2);
    let monitor = Trace::with_monitor(limits, RejectMonitor::default());
    let mut simulation = Simulation::new(
        TimelineId::new(84),
        StoppingWorld,
        topology([1]),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("traced simulation must build: {error}"))
    .with_monitor(monitor);
    let Err(error) = simulation.step() else {
        panic!("composed monitor must fail");
    };

    assert!(!error.to_string().is_empty());
    assert!(matches!(
        error,
        StepError::Monitor {
            source: TraceError::Monitor(MonitorFailure),
            ..
        }
    ));
    let trace = simulation.monitor();
    assert_eq!(trace.limits(), limits);
    assert_eq!(trace.snapshot().limits(), limits);
    assert_eq!(trace.snapshot().entries(), 1);
    assert_eq!(trace.snapshot().retained_bytes(), RetainedBytes::ZERO);
    assert_eq!(trace.entries().len(), 1);
    assert_eq!(trace.monitor().calls, 1);

    let monitor = Trace::with_monitor(limits, RejectMonitor::default()).into_monitor();
    assert_eq!(monitor.calls, 0);
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

fn recorded_action() -> (Replay, TraceEntry) {
    let mut traced = Simulation::new(
        TimelineId::new(83),
        StoppingWorld,
        topology([1]),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("trace source must build: {error}"))
    .with_monitor(Trace::new(trace_limits(2)));
    traced
        .run_to_completion()
        .unwrap_or_else(|error| panic!("trace source must complete: {error}"));
    let entry = traced
        .monitor()
        .entries()
        .next()
        .copied()
        .unwrap_or_else(|| panic!("trace source must retain one action"));
    (traced.monitor().replay(), entry)
}

fn replay_world(
    now: Moment,
    topology: Topology,
    replay: Replay,
) -> Simulation<StoppingWorld, Replay> {
    Simulation::with_parts(
        TimelineId::new(83),
        now,
        StoppingWorld,
        topology,
        SimulationLimits::default(),
        replay,
        NoopMonitor::new(),
    )
    .unwrap_or_else(|error| panic!("replay world must build: {error}"))
}

fn trace_limits(entries: usize) -> TraceLimits {
    TraceLimits::new(nonzero(entries), RetainedBytes::ZERO)
}

fn topology<const N: usize>(duties: [u32; N]) -> Topology {
    Topology::new(duties.map(DutyId::new))
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
