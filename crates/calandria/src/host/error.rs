//! Exact failure boundaries for one embedded host step.

use core::fmt;

use crate::Moment;

use super::HostPhase;

/// Failure to execute one bounded host step.
#[derive(Debug)]
pub enum HostError<DutyError, ClockError> {
    /// The host was already terminal and cannot execute another turn.
    NotRunning {
        /// Existing terminal phase.
        phase: HostPhase,
    },
    /// The configured clock failed to produce an observation.
    Clock(ClockError),
    /// The configured clock moved backward.
    ClockRegressed {
        /// Last accepted moment.
        previous: Moment,
        /// Rejected later observation.
        observed: Moment,
    },
    /// The duty failed during its bounded turn.
    Duty(DutyError),
}

impl<DutyError, ClockError> HostError<DutyError, ClockError> {
    /// Returns the existing phase for a rejected terminal step.
    pub const fn phase(&self) -> Option<HostPhase> {
        match self {
            Self::NotRunning { phase } => Some(*phase),
            Self::Clock(_) | Self::ClockRegressed { .. } | Self::Duty(_) => None,
        }
    }
}

impl<DutyError, ClockError> fmt::Display for HostError<DutyError, ClockError>
where
    DutyError: fmt::Display,
    ClockError: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRunning { phase } => write!(formatter, "host is already {phase:?}"),
            Self::Clock(source) => write!(formatter, "host clock failed: {source}"),
            Self::ClockRegressed { previous, observed } => write!(
                formatter,
                "host clock regressed from {}ns to {}ns",
                previous.as_nanos(),
                observed.as_nanos()
            ),
            Self::Duty(source) => write!(formatter, "host duty failed: {source}"),
        }
    }
}

impl<DutyError, ClockError> core::error::Error for HostError<DutyError, ClockError>
where
    DutyError: core::error::Error + 'static,
    ClockError: core::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Clock(source) => Some(source),
            Self::Duty(source) => Some(source),
            Self::NotRunning { .. } | Self::ClockRegressed { .. } => None,
        }
    }
}
