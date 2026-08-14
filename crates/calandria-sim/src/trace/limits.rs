//! Hard count and retained-byte limits for one causal trace.

use core::num::NonZeroUsize;

use calandria::{EventBatchLimits, RetainedBytes};

/// Maximum entries and variable bytes retained by one causal trace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraceLimits {
    entries: NonZeroUsize,
    retained_bytes: RetainedBytes,
}

impl TraceLimits {
    /// Creates explicit trace limits.
    ///
    /// Trace entries retain fixed-width facts and therefore measure zero
    /// variable bytes. A zero byte limit states that contract exactly.
    pub const fn new(entries: NonZeroUsize, retained_bytes: RetainedBytes) -> Self {
        Self {
            entries,
            retained_bytes,
        }
    }

    /// Returns the maximum retained entry count.
    pub const fn entries(self) -> NonZeroUsize {
        self.entries
    }

    /// Returns the maximum variable retained bytes.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.retained_bytes
    }

    pub(crate) const fn batch(self) -> EventBatchLimits {
        EventBatchLimits::new(self.entries, self.retained_bytes)
    }
}
