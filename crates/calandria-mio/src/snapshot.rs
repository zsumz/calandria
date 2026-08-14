//! Immutable adapter ownership observations.

use crate::MioPollerLimits;

/// Current registration and backend-token state for one Mio poller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MioPollerSnapshot {
    limits: MioPollerLimits,
    registrations: usize,
    backend_tokens_issued: usize,
    token_space_exhausted: bool,
}

impl MioPollerSnapshot {
    pub(crate) const fn new(
        limits: MioPollerLimits,
        registrations: usize,
        next_backend_token: Option<usize>,
    ) -> Self {
        let (backend_tokens_issued, token_space_exhausted) = match next_backend_token {
            Some(next) => (next.saturating_sub(1), false),
            None => (usize::MAX, true),
        };
        Self {
            limits,
            registrations,
            backend_tokens_issued,
            token_space_exhausted,
        }
    }

    /// Returns configured hard limits.
    pub const fn limits(self) -> MioPollerLimits {
        self.limits
    }

    /// Returns the active registration count.
    pub const fn registrations(self) -> usize {
        self.registrations
    }

    /// Returns the number of backend token identities already issued.
    pub const fn backend_tokens_issued(self) -> usize {
        self.backend_tokens_issued
    }

    /// Returns whether the backend token domain is exhausted.
    pub const fn token_space_exhausted(self) -> bool {
        self.token_space_exhausted
    }
}
