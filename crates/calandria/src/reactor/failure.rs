//! Exact terminal reasons for one reactor run loop.

use core::fmt;

use crate::HostError;

/// Failure that terminally ended one reactor loop.
#[derive(Debug)]
pub enum ReactorFailure<DutyError, ClockError, WaitError> {
    /// Clock, duty, regression, or invalid-phase failure from the host.
    Host(HostError<DutyError, ClockError>),
    /// Failure returned by the configured waiting backend.
    Wait(WaitError),
}

impl<DE, CE, WE> fmt::Display for ReactorFailure<DE, CE, WE>
where
    DE: fmt::Display,
    CE: fmt::Display,
    WE: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host(source) => write!(formatter, "reactor host failed: {source}"),
            Self::Wait(source) => write!(formatter, "reactor wait failed: {source}"),
        }
    }
}

impl<DE, CE, WE> core::error::Error for ReactorFailure<DE, CE, WE>
where
    DE: core::error::Error + 'static,
    CE: core::error::Error + 'static,
    WE: core::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Host(source) => Some(source),
            Self::Wait(source) => Some(source),
        }
    }
}

/// Terminal reason published by one reactor owner.
#[derive(Debug)]
pub enum ReactorOutcome<DutyError, ClockError, WaitError> {
    /// The duty returned [`crate::Next::Stop`].
    Stopped,
    /// The framework's explicit fail-safe termination was observed.
    Terminated,
    /// The host or waiting backend failed before a clean stop.
    Failed(ReactorFailure<DutyError, ClockError, WaitError>),
}
