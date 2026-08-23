//! Single-consumer blocking, nonblocking, and future observation.

use std::{
    cell::Cell,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use super::{CompletionError, shared::Shared};

/// A single-consumer handle for one eventual terminal value.
///
/// Dropping or explicitly abandoning this handle only abandons observation. It
/// does not cancel producer work. The producer receives its value back if it
/// later attempts to publish.
///
/// The handle is `Send` when `T` is `Send`, but deliberately not `Sync`; one
/// observer owns terminal extraction.
#[must_use = "dropping a completion abandons observation; producer work may continue"]
pub struct Completion<T> {
    pub(super) shared: Shared<T>,
    pub(super) _single_observer: PhantomData<Cell<()>>,
}

impl<T> Completion<T> {
    /// Blocks the current thread until a value arrives or the producer closes.
    pub fn wait(self) -> Result<T, CompletionError> {
        self.shared.wait()
    }

    /// Takes the terminal result without blocking, or returns `None` while pending.
    ///
    /// A returned `Some` consumes the single outcome. Later extraction or
    /// polling reports [`CompletionError::Consumed`].
    pub fn try_take(&self) -> Option<Result<T, CompletionError>> {
        self.shared.try_take()
    }

    /// Polls for and consumes the terminal result using one exclusive observer.
    ///
    /// Mutable access preserves one active task-waker registration. Repeated
    /// pending polls replace a stale waker; a ready result is consumed once.
    pub fn poll_take(&mut self, context: &mut Context<'_>) -> Poll<Result<T, CompletionError>> {
        self.shared.poll(context)
    }

    /// Explicitly abandons observation without cancelling producer work.
    pub fn abandon(self) {
        drop(self);
    }
}

impl<T> Future for Completion<T> {
    type Output = Result<T, CompletionError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().poll_take(context)
    }
}

impl<T> fmt::Debug for Completion<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Completion").finish_non_exhaustive()
    }
}

impl<T> Drop for Completion<T> {
    fn drop(&mut self) {
        self.shared.abandon_observer();
    }
}
