//! Unique consumer side of the bounded reactor mailbox.

use std::{cell::Cell, fmt, marker::PhantomData, num::NonZeroUsize, sync::Arc};

use crate::WakeHandle;

use super::{DrainReport, DrainStatus, MailboxSnapshot, shared::Shared};

/// Single owner of mailbox consumption and wake acknowledgement.
///
/// The receiver is movable between threads but deliberately not `Sync`.
pub struct MailboxReceiver<T> {
    pub(super) shared: Arc<Shared<T>>,
    pub(super) _single_consumer: PhantomData<Cell<()>>,
}

impl<T> MailboxReceiver<T> {
    /// Drains at most `limit` values, taking control work before ordinary work.
    pub fn drain_into(
        &mut self,
        destination: &mut Vec<T>,
        limit: NonZeroUsize,
    ) -> DrainReport {
        let mut state = self.shared.lock();
        let before = destination.len();

        let controls = limit.get().min(state.control.queue.len());
        state.control.drain_into(controls, destination);
        let remaining = limit.get() - controls;
        let work = remaining.min(state.work.queue.len());
        state.work.drain_into(work, destination);

        let status = if state.is_empty() {
            self.shared.wake.acknowledge();
            if !state.receiver_alive || state.senders == 0 {
                DrainStatus::Closed
            } else {
                DrainStatus::Idle
            }
        } else {
            DrainStatus::MorePending
        };

        DrainReport {
            drained: destination.len() - before,
            status,
        }
    }

    /// Closes admission and returns every retained item in drain order.
    pub fn close(&mut self) -> Vec<T> {
        self.close_inner()
    }

    /// Returns the mailbox wake handle for integration with an owner host.
    pub fn wake_handle(&self) -> WakeHandle {
        self.shared.wake.clone()
    }

    /// Returns a current and cumulative pressure snapshot.
    pub fn snapshot(&self) -> MailboxSnapshot {
        self.shared.snapshot()
    }

    fn close_inner(&mut self) -> Vec<T> {
        let mut state = self.shared.lock();
        state.receiver_alive = false;
        self.shared.wake.acknowledge();
        let mut items = Vec::with_capacity(state.control.queue.len() + state.work.queue.len());
        state.control.drain_all(&mut items);
        state.work.drain_all(&mut items);
        items
    }
}

impl<T> Drop for MailboxReceiver<T> {
    fn drop(&mut self) {
        drop(self.close_inner());
    }
}

impl<T> fmt::Debug for MailboxReceiver<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MailboxReceiver")
            .field("limits", &self.shared.limits)
            .finish_non_exhaustive()
    }
}
