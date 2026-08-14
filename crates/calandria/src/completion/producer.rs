//! Unique authority for publishing or closing one completion.

use std::fmt;

use super::shared::Shared;

/// Single-use producer for one terminal completion value.
#[must_use = "dropping a completer closes its pending completion"]
pub struct Completer<T> {
    pub(super) shared: Shared<T>,
    pub(super) settled: bool,
}

impl<T> Completer<T> {
    /// Publishes the terminal value exactly once.
    ///
    /// If the observer has already been abandoned, ownership of `value` is
    /// returned and no terminal value is retained.
    pub fn complete(mut self, value: T) -> Result<(), T> {
        let result = self.shared.complete(value);
        self.settled = true;
        result
    }

    /// Explicitly closes the completion without a value.
    pub fn close(mut self) {
        self.shared.close_producer();
        self.settled = true;
    }
}

impl<T> fmt::Debug for Completer<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Completer")
            .field("settled", &self.settled)
            .finish_non_exhaustive()
    }
}

impl<T> Drop for Completer<T> {
    fn drop(&mut self) {
        if !self.settled {
            self.shared.close_producer();
        }
    }
}
