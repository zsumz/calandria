//! Observable result of requesting bounded reactor termination.

use std::io;

/// Lifecycle state observed by one termination request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReactorTerminationStatus {
    /// This call published the first termination request.
    Requested,
    /// A previous call already published termination.
    AlreadyRequested,
    /// The reactor had already exited.
    Exited,
}

/// Result of one reactor termination request.
///
/// A wake failure does not retract a first request. The reactor still observes
/// termination through its configured bounded wait ceiling.
#[must_use = "termination can carry an unobserved best-effort wake failure"]
#[derive(Debug)]
pub struct ReactorTermination {
    status: ReactorTerminationStatus,
    wake_error: Option<io::Error>,
}

impl ReactorTermination {
    pub(super) const fn new(
        status: ReactorTerminationStatus,
        wake_error: Option<io::Error>,
    ) -> Self {
        Self { status, wake_error }
    }

    /// Returns the lifecycle state observed by this request.
    pub const fn status(&self) -> ReactorTerminationStatus {
        self.status
    }

    /// Returns a failed best-effort wake associated with an accepted request.
    pub const fn wake_error(&self) -> Option<&io::Error> {
        self.wake_error.as_ref()
    }

    /// Consumes the result and returns a failed best-effort wake, if any.
    pub fn into_wake_error(self) -> Option<io::Error> {
        self.wake_error
    }
}
