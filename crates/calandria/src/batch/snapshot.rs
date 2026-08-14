//! Immutable event-batch ownership observations.

use crate::RetainedBytes;

use super::EventBatchLimits;

/// Current bounded ownership for one event batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventBatchSnapshot {
    limits: EventBatchLimits,
    events: usize,
    retained_bytes: RetainedBytes,
}

impl EventBatchSnapshot {
    pub(super) const fn new(
        limits: EventBatchLimits,
        events: usize,
        retained_bytes: RetainedBytes,
    ) -> Self {
        Self {
            limits,
            events,
            retained_bytes,
        }
    }

    /// Returns configured hard limits.
    pub const fn limits(self) -> EventBatchLimits {
        self.limits
    }

    /// Returns the retained event count.
    pub const fn events(self) -> usize {
        self.events
    }

    /// Returns variable bytes retained by batch events.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.retained_bytes
    }
}
