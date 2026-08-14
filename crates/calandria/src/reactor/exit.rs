//! Owned terminal state returned by one reactor run loop.

use crate::{Clock, Duty, EmbeddedHost, HostSnapshot, Waiter};

use super::{ReactorOutcome, ReactorSnapshot};

type ReactorParts<D, C, W> = (
    EmbeddedHost<D, C>,
    W,
    ReactorOutcome<<D as Duty>::Error, <C as Clock>::Error, <W as Waiter<D>>::Error>,
    ReactorSnapshot,
);

/// Owned terminal state returned by one reactor run loop.
#[derive(Debug)]
pub struct ReactorExit<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    pub(super) host: EmbeddedHost<D, C>,
    pub(super) waiter: W,
    pub(super) outcome: ReactorOutcome<D::Error, C::Error, W::Error>,
    pub(super) snapshot: ReactorSnapshot,
}

impl<D, C, W> ReactorExit<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    /// Returns the terminal reason.
    pub const fn outcome(&self) -> &ReactorOutcome<D::Error, C::Error, W::Error> {
        &self.outcome
    }

    /// Returns bounded host observations.
    pub const fn host_snapshot(&self) -> HostSnapshot {
        self.host.snapshot()
    }

    /// Returns bounded wait-loop observations.
    pub const fn reactor_snapshot(&self) -> ReactorSnapshot {
        self.snapshot
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
    pub fn into_parts(self) -> ReactorParts<D, C, W> {
        (self.host, self.waiter, self.outcome, self.snapshot)
    }

    /// Consumes the exit and returns the terminal duty.
    pub fn into_duty(self) -> D {
        self.host.into_duty()
    }
}
