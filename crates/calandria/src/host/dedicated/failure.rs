//! Terminal outcomes for a dedicated host loop.

use core::fmt;

use super::super::HostError;

/// Failure that terminally ended a dedicated host loop.
#[derive(Debug)]
pub enum DedicatedFailure<DutyError, ClockError, WaitError> {
    /// Clock, duty, regression, or invalid-phase failure from the embedded host.
    Host(HostError<DutyError, ClockError>),
    /// Failure returned by the configured waiting backend.
    Wait(WaitError),
}

impl<DutyError, ClockError, WaitError> fmt::Display
    for DedicatedFailure<DutyError, ClockError, WaitError>
where
    DutyError: fmt::Display,
    ClockError: fmt::Display,
    WaitError: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host(source) => write!(formatter, "dedicated host failed: {source}"),
            Self::Wait(source) => write!(formatter, "dedicated host wait failed: {source}"),
        }
    }
}

impl<DutyError, ClockError, WaitError> core::error::Error
    for DedicatedFailure<DutyError, ClockError, WaitError>
where
    DutyError: core::error::Error + 'static,
    ClockError: core::error::Error + 'static,
    WaitError: core::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Host(source) => Some(source),
            Self::Wait(source) => Some(source),
        }
    }
}

/// Terminal reason published by a dedicated host thread.
#[derive(Debug)]
pub enum DedicatedOutcome<DutyError, ClockError, WaitError> {
    /// The duty returned [`crate::Next::Stop`].
    Stopped,
    /// The host failed before the duty stopped.
    Failed(DedicatedFailure<DutyError, ClockError, WaitError>),
}
