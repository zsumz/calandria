//! Host policy that does not belong to a concrete duty.

use crate::Span;

/// Policy for turning complete scheduling interest into bounded waiting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostConfig {
    wait_ceiling: Span,
}

impl HostConfig {
    /// Default maximum wait between owner observations: 100 milliseconds.
    pub const DEFAULT_WAIT_CEILING: Span = Span::from_nanos(100_000_000);

    /// Creates host policy with an explicit maximum wait span.
    ///
    /// A zero ceiling deliberately requests polling without blocking.
    pub const fn new(wait_ceiling: Span) -> Self {
        Self { wait_ceiling }
    }

    /// Returns the maximum span used for one wait attempt.
    pub const fn wait_ceiling(self) -> Span {
        self.wait_ceiling
    }
}

impl Default for HostConfig {
    fn default() -> Self {
        Self::new(Self::DEFAULT_WAIT_CEILING)
    }
}
