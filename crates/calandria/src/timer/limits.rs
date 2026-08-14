//! Hard count and retained-byte limits for one timer queue.

use core::num::NonZeroUsize;

use crate::RetainedBytes;

/// Maximum timers and variable bytes retained by one owner-local queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerLimits {
    timers: NonZeroUsize,
    retained_bytes: RetainedBytes,
}

impl TimerLimits {
    /// Creates timer-queue limits.
    ///
    /// A zero retained-byte limit accepts only fixed-size timer values whose
    /// [`crate::Retained`] measurement is zero.
    pub const fn new(timers: NonZeroUsize, retained_bytes: RetainedBytes) -> Self {
        Self {
            timers,
            retained_bytes,
        }
    }

    /// Returns the maximum pending timer count.
    pub const fn timers(self) -> NonZeroUsize {
        self.timers
    }

    /// Returns the maximum variable retained bytes.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.retained_bytes
    }
}

impl Default for TimerLimits {
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
        None => panic!("timer default count must be nonzero"),
    }
}
