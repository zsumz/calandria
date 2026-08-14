//! Live static reactor-group ownership and terminal observation.

use std::{
    num::NonZeroUsize,
    thread::{self, JoinHandle, Thread},
};

use crate::{Clock, Duty, Waiter};

use super::{
    GroupControl, ReactorGroupExit, ReactorGroupHandle, ReactorGroupLimits,
    ReactorGroupTermination, ReactorId, Supervised,
};

/// Live static group of independently owned reactor threads.
#[must_use = "dropping the group detaches observation and does not terminate its reactors"]
pub struct ReactorGroup<D, C, W, T>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    pub(super) limits: ReactorGroupLimits,
    pub(super) reactors: NonZeroUsize,
    pub(super) ingress: ReactorGroupHandle<T>,
    pub(super) control: GroupControl<T>,
    pub(super) threads: Box<[Thread]>,
    pub(super) supervisor: JoinHandle<Supervised<D, C, W>>,
}

impl<D, C, W, T> ReactorGroup<D, C, W, T>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    /// Returns the configured hard topology limit.
    pub const fn limits(&self) -> ReactorGroupLimits {
        self.limits
    }

    /// Returns the immutable number of reactors.
    pub const fn reactors(&self) -> NonZeroUsize {
        self.reactors
    }

    /// Creates another bounded typed ingress handle.
    pub fn handle(&self) -> ReactorGroupHandle<T> {
        self.ingress.clone()
    }

    /// Closes ingress, publishes every termination, then runs best-effort wakes.
    pub fn request_termination(&self) -> ReactorGroupTermination {
        self.control.request_termination()
    }

    /// Returns one underlying reactor thread handle.
    pub fn reactor_thread(&self, reactor: ReactorId) -> Option<&Thread> {
        reactor
            .position()
            .and_then(|position| self.threads.get(position))
    }

    /// Returns whether the supervisor has collected every reactor exit.
    pub fn is_finished(&self) -> bool {
        self.supervisor.is_finished()
    }

    /// Joins the supervisor and returns every ordered member exit.
    pub fn join(self) -> thread::Result<ReactorGroupExit<D, C, W>> {
        self.supervisor.join().map(ReactorGroupExit::new)
    }
}

impl<D, C, W, T> core::fmt::Debug for ReactorGroup<D, C, W, T>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ReactorGroup")
            .field("limits", &self.limits)
            .field("reactors", &self.reactors)
            .field("ingress", &self.ingress)
            .field("finished", &self.is_finished())
            .finish_non_exhaustive()
    }
}
