//! One-step scheduler, model, monitor, and lifecycle failures.

use core::fmt;

use crate::{ActionKey, ActionMeta, SimulationPhase};

use super::{KernelFailure, LimitFailure};

/// Why one deterministic kernel step could not complete.
#[derive(Debug)]
pub enum StepError<ModelError, MonitorError, SchedulerError> {
    /// Consumer model action failed after its staged effects rolled back.
    Model {
        action: ActionMeta,
        source: ModelError,
    },
    /// A post-commit monitor rejected the resulting state.
    Monitor {
        action: ActionMeta,
        source: MonitorError,
    },
    /// Scheduler policy failed before model execution.
    Scheduler(SchedulerError),
    /// Scheduler returned an action outside the supplied ready set.
    InvalidSelection(ActionKey),
    /// A kernel ownership or lifecycle invariant failed.
    Kernel(KernelFailure),
    /// A hard execution limit prevented another transition.
    Limit(LimitFailure),
    /// Panic unwinding crossed a model, monitor, or scheduler boundary.
    Poisoned,
    /// The simulation is no longer active.
    Inactive(SimulationPhase),
}

impl<ME: fmt::Display, NE: fmt::Display, SE: fmt::Display> fmt::Display
    for StepError<ME, NE, SE>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model { action, source } => write!(
                formatter,
                "model action {} failed: {source}",
                action.id().get()
            ),
            Self::Monitor { action, source } => write!(
                formatter,
                "monitor rejected action {}: {source}",
                action.id().get()
            ),
            Self::Scheduler(source) => write!(formatter, "scheduler failed: {source}"),
            Self::InvalidSelection(action) => write!(
                formatter,
                "scheduler selected unavailable action for duty {}",
                action.duty().get()
            ),
            Self::Kernel(failure) => failure.fmt(formatter),
            Self::Limit(failure) => failure.fmt(formatter),
            Self::Poisoned => formatter.write_str("simulation is poisoned"),
            Self::Inactive(phase) => write!(formatter, "simulation is {phase:?}"),
        }
    }
}

impl<ME, NE, SE> core::error::Error for StepError<ME, NE, SE>
where
    ME: fmt::Debug + fmt::Display,
    NE: fmt::Debug + fmt::Display,
    SE: fmt::Debug + fmt::Display,
{
}
