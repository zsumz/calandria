//! Unique terminal authority for one shutdown barrier.

use std::{fmt, sync::Arc};

use super::shared::{Phase, Shared};

/// Unique authority for settling one shutdown barrier.
#[must_use = "dropping an unsettled shutdown completer closes the barrier"]
pub struct ShutdownCompleter {
    pub(super) shared: Arc<Shared>,
    pub(super) settled: bool,
}

impl ShutdownCompleter {
    /// Publishes successful shutdown to every admitted observer exactly once.
    ///
    /// Repeated calls are harmless. Late subscribers receive an already-ready
    /// completion without consuming subscriber capacity.
    pub fn complete(&mut self) {
        if self.settled {
            return;
        }
        self.settled = true;
        let subscribers = self.shared.settle(Phase::Completed);
        for subscriber in subscribers {
            let _ = subscriber.complete(());
        }
    }

    /// Explicitly closes the barrier without publishing successful shutdown.
    ///
    /// Every retained observer receives
    /// [`CompletionError::Closed`](crate::CompletionError::Closed),
    /// and future subscriptions are rejected. Dropping an unsettled completer has
    /// the same fail-closed result.
    pub fn close(mut self) {
        self.settled = true;
        drop(self.shared.settle(Phase::Closed));
    }

    /// Returns whether successful shutdown was already published.
    pub const fn is_settled(&self) -> bool {
        self.settled
    }
}

impl fmt::Debug for ShutdownCompleter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ShutdownCompleter")
            .field("settled", &self.settled)
            .finish_non_exhaustive()
    }
}

impl Drop for ShutdownCompleter {
    fn drop(&mut self) {
        if !self.settled {
            drop(self.shared.settle(Phase::Closed));
        }
    }
}
