//! Immutable virtual-time and pending-ownership observations.

use calandria::{Moment, RetainedBytes};

use super::TimelineLimits;
use crate::TimelineId;

/// Current bounded state for one timeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimelineSnapshot {
    id: TimelineId,
    limits: TimelineLimits,
    now: Moment,
    pending_events: usize,
    retained_bytes: RetainedBytes,
    next_at: Option<Moment>,
}

impl TimelineSnapshot {
    pub(super) const fn new(
        id: TimelineId,
        limits: TimelineLimits,
        now: Moment,
        pending_events: usize,
        retained_bytes: RetainedBytes,
        next_at: Option<Moment>,
    ) -> Self {
        Self {
            id,
            limits,
            now,
            pending_events,
            retained_bytes,
            next_at,
        }
    }

    /// Returns the timeline identity.
    pub const fn id(self) -> TimelineId {
        self.id
    }

    /// Returns configured hard limits.
    pub const fn limits(self) -> TimelineLimits {
        self.limits
    }

    /// Returns current virtual time.
    pub const fn now(self) -> Moment {
        self.now
    }

    /// Returns the number of pending events.
    pub const fn pending_events(self) -> usize {
        self.pending_events
    }

    /// Returns variable bytes retained by pending events.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.retained_bytes
    }

    /// Returns the earliest pending event moment.
    pub const fn next_at(self) -> Option<Moment> {
        self.next_at
    }
}
