//! Timer drain results and bounded ownership observations.

use crate::{Deadline, RetainedBytes};

use super::{TimerId, TimerLimits, TimerOwnerId};

/// Progress made by one explicitly bounded due-timer drain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerDrain {
    fired: usize,
    more_due: bool,
}

impl TimerDrain {
    pub(super) const fn new(fired: usize, more_due: bool) -> Self {
        Self { fired, more_due }
    }

    /// Returns the number of timers transferred to the destination.
    pub const fn fired(self) -> usize {
        self.fired
    }

    /// Returns whether another timer is already due at the supplied moment.
    pub const fn more_due(self) -> bool {
        self.more_due
    }
}

/// Current bounded state for one owner-local timer queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerQueueSnapshot {
    owner: TimerOwnerId,
    limits: TimerLimits,
    pending_timers: usize,
    retained_bytes: RetainedBytes,
    next_deadline: Option<Deadline>,
    next_id: Option<TimerId>,
}

impl TimerQueueSnapshot {
    pub(super) const fn new(
        owner: TimerOwnerId,
        limits: TimerLimits,
        pending_timers: usize,
        retained_bytes: RetainedBytes,
        next_deadline: Option<Deadline>,
        next_id: Option<TimerId>,
    ) -> Self {
        Self {
            owner,
            limits,
            pending_timers,
            retained_bytes,
            next_deadline,
            next_id,
        }
    }

    /// Returns the queue owner identity.
    pub const fn owner(self) -> TimerOwnerId {
        self.owner
    }

    /// Returns configured hard limits.
    pub const fn limits(self) -> TimerLimits {
        self.limits
    }

    /// Returns the pending timer count.
    pub const fn pending_timers(self) -> usize {
        self.pending_timers
    }

    /// Returns variable bytes retained by timer values.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.retained_bytes
    }

    /// Returns the earliest absolute deadline.
    pub const fn next_deadline(self) -> Option<Deadline> {
        self.next_deadline
    }

    /// Returns the identity the next successful admission would consume.
    pub const fn next_id(self) -> Option<TimerId> {
        self.next_id
    }

    /// Returns whether no further timer identity can be issued.
    pub const fn identities_exhausted(self) -> bool {
        self.next_id.is_none()
    }
}
