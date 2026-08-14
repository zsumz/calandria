//! Owned terminal state returned by a dedicated host thread.

use super::{DedicatedOutcome, DedicatedSnapshot};
use crate::host::{Clock, Duty, EmbeddedHost, HostSnapshot, Waiter};

/// Owned terminal state returned by a dedicated host thread.
#[derive(Debug)]
pub struct DedicatedExit<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    pub(super) host: EmbeddedHost<D, C>,
    pub(super) waiter: W,
    pub(super) outcome: DedicatedOutcome<D::Error, C::Error, W::Error>,
    pub(super) dedicated: DedicatedSnapshot,
}

impl<D, C, W> DedicatedExit<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    /// Returns the terminal reason.
    pub const fn outcome(&self) -> &DedicatedOutcome<D::Error, C::Error, W::Error> {
        &self.outcome
    }

    /// Returns embedded-turn observations.
    pub const fn host_snapshot(&self) -> HostSnapshot {
        self.host.snapshot()
    }

    /// Returns dedicated-wait observations.
    pub const fn dedicated_snapshot(&self) -> DedicatedSnapshot {
        self.dedicated
    }

    /// Returns shared access to the terminal duty.
    pub const fn duty(&self) -> &D {
        self.host.duty()
    }

    /// Returns shared access to the terminal waiting backend.
    pub const fn waiter(&self) -> &W {
        &self.waiter
    }

    /// Consumes the exit and returns every owned component and observation.
    pub fn into_parts(
        self,
    ) -> (
        EmbeddedHost<D, C>,
        W,
        DedicatedOutcome<D::Error, C::Error, W::Error>,
        DedicatedSnapshot,
    ) {
        (self.host, self.waiter, self.outcome, self.dedicated)
    }

    /// Consumes the exit and returns the terminal duty.
    pub fn into_duty(self) -> D {
        self.host.into_duty()
    }
}
