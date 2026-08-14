//! Hard count and retained-byte limits for one event batch.

use core::num::NonZeroUsize;

use crate::RetainedBytes;

/// Maximum values and variable bytes retained by one owner-local event batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventBatchLimits {
    events: NonZeroUsize,
    retained_bytes: RetainedBytes,
}

impl EventBatchLimits {
    /// Creates event-batch limits.
    ///
    /// A zero retained-byte limit accepts only fixed-size events whose
    /// [`crate::Retained`] measurement is zero.
    pub const fn new(events: NonZeroUsize, retained_bytes: RetainedBytes) -> Self {
        Self {
            events,
            retained_bytes,
        }
    }

    /// Returns the maximum retained event count.
    pub const fn events(self) -> NonZeroUsize {
        self.events
    }

    /// Returns the maximum variable retained bytes.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.retained_bytes
    }
}
