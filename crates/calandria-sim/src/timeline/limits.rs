//! Hard event-count and retained-byte limits.

use core::num::NonZeroUsize;

use calandria::RetainedBytes;

/// Hard count and retained-byte limits for one event timeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimelineLimits {
    pending_events: NonZeroUsize,
    retained_bytes: RetainedBytes,
}

impl TimelineLimits {
    /// Creates timeline limits.
    ///
    /// A zero retained-byte limit accepts only fixed-size events whose
    /// [`calandria::Retained`] measurement is zero.
    pub const fn new(pending_events: NonZeroUsize, retained_bytes: RetainedBytes) -> Self {
        Self {
            pending_events,
            retained_bytes,
        }
    }

    /// Returns the maximum pending event count.
    pub const fn pending_events(self) -> NonZeroUsize {
        self.pending_events
    }

    /// Returns the maximum variable retained bytes.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.retained_bytes
    }
}

impl Default for TimelineLimits {
    fn default() -> Self {
        Self::new(
            nonzero_usize(1_024),
            RetainedBytes::new(16 * 1_024 * 1_024),
        )
    }
}

const fn nonzero_usize(value: usize) -> NonZeroUsize {
    match NonZeroUsize::new(value) {
        Some(value) => value,
        None => panic!("timeline default count must be nonzero"),
    }
}
