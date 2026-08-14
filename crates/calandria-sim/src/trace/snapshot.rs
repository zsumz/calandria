//! Immutable retained ownership observations for one causal trace.

use calandria::{EventBatchSnapshot, RetainedBytes};

use super::TraceLimits;

/// Current bounded ownership retained by one causal trace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraceSnapshot {
    limits: TraceLimits,
    entries: usize,
    retained_bytes: RetainedBytes,
}

impl TraceSnapshot {
    pub(crate) fn from_batch(limits: TraceLimits, batch: EventBatchSnapshot) -> Self {
        Self {
            limits,
            entries: batch.events(),
            retained_bytes: batch.retained_bytes(),
        }
    }

    /// Returns configured hard limits.
    pub const fn limits(self) -> TraceLimits {
        self.limits
    }

    /// Returns the retained trace entry count.
    pub const fn entries(self) -> usize {
        self.entries
    }

    /// Returns variable bytes retained by trace entries.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.retained_bytes
    }
}
