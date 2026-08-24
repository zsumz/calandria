//! Cloneable producer side of the bounded reactor mailbox.

use std::fmt;

use crate::{
    RetainedBytes,
    sync::{Arc, atomic::Ordering},
};

use super::{AdmissionFailure, Lane, MailboxSnapshot, TrySendError, shared::Shared};

/// Cloneable producer for a bounded reactor mailbox.
pub struct MailboxSender<T> {
    pub(super) shared: Arc<Shared<T>>,
}

impl<T> MailboxSender<T> {
    /// Attempts to publish ordinary work.
    pub fn try_send(&self, item: T) -> Result<(), TrySendError<T>> {
        self.try_send_to(Lane::Work, item)
    }

    /// Attempts to publish control work.
    pub fn try_send_control(&self, item: T) -> Result<(), TrySendError<T>> {
        self.try_send_to(Lane::Control, item)
    }

    /// Attempts to publish to a selected lane.
    pub fn try_send_to(&self, lane: Lane, item: T) -> Result<(), TrySendError<T>> {
        let measure = self.shared.measure;
        self.try_send_materialized(lane, item, measure, core::convert::identity)
    }

    /// Admits an owned source value before materializing the queued value.
    ///
    /// Capacity is proven and the owner wake is requested before `materialize`
    /// consumes `owner`. Any rejection returns the original owner unchanged.
    /// `retained_bytes` must report the variable memory that the materialized
    /// queued value will retain. `materialize` runs while the mailbox lock is
    /// held, so it must not reenter this mailbox, wait for work that requires
    /// this mailbox, or invoke arbitrary blocking consumer code. Both callbacks
    /// must be fast, deterministic, and infallible.
    pub fn try_send_materialized<U>(
        &self,
        lane: Lane,
        owner: U,
        retained_bytes: impl FnOnce(&U) -> RetainedBytes,
        materialize: impl FnOnce(U) -> T,
    ) -> Result<(), TrySendError<U>> {
        let retained = retained_bytes(&owner);
        let mut state = self.shared.lock();
        if !state.receiver_alive {
            self.shared.counters.increment_closed();
            return Err(TrySendError::new(owner, lane, AdmissionFailure::Closed));
        }

        let lane_limits = self.shared.limits.lane(lane);
        let lane_state = state.lane(lane);
        if lane_state.queue.len() >= lane_limits.messages().get() {
            self.shared.counters.lane(lane).increment_messages();
            return Err(TrySendError::new(
                owner,
                lane,
                AdmissionFailure::MessageCapacity,
            ));
        }

        let Some(next_bytes) = lane_state.retained.checked_add(retained) else {
            self.shared.counters.lane(lane).increment_bytes();
            return Err(TrySendError::new(
                owner,
                lane,
                AdmissionFailure::ByteCapacity,
            ));
        };
        if next_bytes.get() > lane_limits.retained_bytes().get() {
            self.shared.counters.lane(lane).increment_bytes();
            return Err(TrySendError::new(
                owner,
                lane,
                AdmissionFailure::ByteCapacity,
            ));
        }

        // The state lock prevents acknowledgement until publication below has
        // either succeeded or returned the untouched owner.
        if let Err(source) = self.shared.wake.wake() {
            self.shared.counters.increment_wake_failures();
            return Err(TrySendError::new(
                owner,
                lane,
                AdmissionFailure::Wake(source),
            ));
        }

        let item = materialize(owner);
        state.lane_mut(lane).admit(item, retained, next_bytes);
        Ok(())
    }

    /// Returns a current and cumulative pressure snapshot.
    pub fn snapshot(&self) -> MailboxSnapshot {
        self.shared.snapshot()
    }
}

impl<T> Clone for MailboxSender<T> {
    fn clone(&self) -> Self {
        let mut state = self.shared.lock();
        state.senders = state
            .senders
            .checked_add(1)
            .unwrap_or_else(|| panic!("mailbox sender count exhausted"));
        drop(state);
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<T> Drop for MailboxSender<T> {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        if state.senders == 0 {
            return;
        }
        if state.senders == 1 && state.receiver_alive && self.shared.wake.wake().is_err() {
            self.shared.counters.increment_wake_failures();
        }
        state.senders -= 1;
    }
}

impl<T> fmt::Debug for MailboxSender<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MailboxSender")
            .field("limits", &self.shared.limits)
            .field(
                "wake_failures",
                &self.shared.counters.wake_failures.load(Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}
