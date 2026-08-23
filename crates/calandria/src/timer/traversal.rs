//! Bounded read-only traversal over pending timer ownership.

use super::{Timer, TimerQueue};

impl<T> TimerQueue<T> {
    /// Iterates over pending timers without allocating.
    ///
    /// Traversal is bounded by configured capacity. Order is unspecified;
    /// callers must not infer deadline order from this iterator.
    pub fn iter(&self) -> impl Iterator<Item = &Timer<T>> + '_ {
        self.timers.iter().map(|scheduled| &scheduled.timer)
    }
}
