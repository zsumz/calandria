//! Single-owner stable deadline ordering with eager exact cancellation.

use alloc::{collections::BinaryHeap, vec::Vec};
use core::{mem, num::NonZeroUsize};

use crate::{Deadline, Moment, Retained, RetainedBytes};

use super::scheduled::Scheduled;
use super::{
    Timer, TimerDrain, TimerId, TimerLimits, TimerOwnerId, TimerQueueSnapshot, TimerScheduleError,
    TimerScheduleFailure, TimerToken,
};

/// Stable count- and retained-byte-bounded owner-local timer queue.
#[derive(Debug)]
pub struct TimerQueue<T> {
    owner: TimerOwnerId,
    limits: TimerLimits,
    measure: fn(&T) -> RetainedBytes,
    timers: BinaryHeap<Scheduled<T>>,
    next_id: Option<TimerId>,
    retained: RetainedBytes,
}

impl<T: Retained> TimerQueue<T> {
    /// Creates an empty queue whose first timer identity is zero.
    pub fn new(owner: TimerOwnerId, limits: TimerLimits) -> Self {
        Self::with_measure(owner, limits, T::retained_bytes)
    }

    /// Creates an empty queue whose first successful admission uses `first_id`.
    ///
    /// This supports restored identity floors and deterministic exhaustion
    /// tests. A queue must never restart below an identity that can still be
    /// presented for cancellation.
    pub fn starting_at(owner: TimerOwnerId, limits: TimerLimits, first_id: TimerId) -> Self {
        Self::starting_at_with_measure(owner, limits, first_id, T::retained_bytes)
    }
}

impl<T> TimerQueue<T> {
    /// Creates an empty queue using an explicit retained-byte measurement.
    pub fn with_measure(
        owner: TimerOwnerId,
        limits: TimerLimits,
        measure: fn(&T) -> RetainedBytes,
    ) -> Self {
        Self::starting_at_with_measure(owner, limits, TimerId::ZERO, measure)
    }

    /// Creates a measured queue whose first successful admission uses `first_id`.
    pub fn starting_at_with_measure(
        owner: TimerOwnerId,
        limits: TimerLimits,
        first_id: TimerId,
        measure: fn(&T) -> RetainedBytes,
    ) -> Self {
        Self {
            owner,
            limits,
            measure,
            timers: BinaryHeap::with_capacity(limits.timers().get()),
            next_id: Some(first_id),
            retained: RetainedBytes::ZERO,
        }
    }

    /// Returns this queue's stable owner identity.
    pub const fn owner(&self) -> TimerOwnerId {
        self.owner
    }

    /// Returns configured hard limits.
    pub const fn limits(&self) -> TimerLimits {
        self.limits
    }

    /// Returns whether no timers are pending.
    pub fn is_empty(&self) -> bool {
        self.timers.is_empty()
    }

    /// Returns the pending timer count.
    pub fn len(&self) -> usize {
        self.timers.len()
    }

    /// Returns the earliest deadline without changing ownership.
    pub fn next_deadline(&self) -> Option<Deadline> {
        self.timers
            .peek()
            .map(|scheduled| scheduled.timer.deadline())
    }

    /// Admits one value at an absolute deadline.
    pub fn schedule(
        &mut self,
        deadline: Deadline,
        value: T,
    ) -> Result<TimerToken, TimerScheduleError<T>> {
        if self.timers.len() >= self.limits.timers().get() {
            return Err(TimerScheduleError::new(
                deadline,
                value,
                TimerScheduleFailure::TimerCapacity {
                    limit: self.limits.timers(),
                },
            ));
        }

        let retained = (self.measure)(&value);
        let next_retained = self.check_retained(deadline, value, retained)?;
        let Some(id) = self.next_id else {
            return Err(TimerScheduleError::new(
                deadline,
                next_retained.value,
                TimerScheduleFailure::TimerIdsExhausted,
            ));
        };
        let token = TimerToken::new(self.owner, id, deadline);
        self.next_id = id.checked_next();
        self.timers.push(Scheduled {
            timer: Timer::new(token, next_retained.value, retained),
        });
        self.retained = next_retained.total;
        Ok(token)
    }

    /// Cancels exactly the named pending timer and returns ownership.
    pub fn cancel(&mut self, token: TimerToken) -> Option<Timer<T>> {
        if token.owner() != self.owner {
            return None;
        }
        let mut timers = mem::take(&mut self.timers).into_vec();
        let Some(index) = timers
            .iter()
            .position(|scheduled| scheduled.timer.token() == token)
        else {
            self.timers = BinaryHeap::from(timers);
            return None;
        };
        let scheduled = timers.swap_remove(index);
        self.timers = BinaryHeap::from(timers);
        self.subtract_retained(scheduled.timer.measured_retained_bytes());
        Some(scheduled.timer)
    }

    /// Removes the first timer when it is due at `now`.
    pub fn pop_due(&mut self, now: Moment) -> Option<Timer<T>> {
        if !self
            .next_deadline()
            .is_some_and(|deadline| deadline.is_elapsed_at(now))
        {
            return None;
        }
        let scheduled = self.timers.pop()?;
        self.subtract_retained(scheduled.timer.measured_retained_bytes());
        Some(scheduled.timer)
    }

    /// Transfers no more than `budget` due timers in stable order.
    pub fn drain_due_into(
        &mut self,
        now: Moment,
        destination: &mut Vec<Timer<T>>,
        budget: NonZeroUsize,
    ) -> TimerDrain {
        let mut fired = 0;
        while fired < budget.get() {
            let Some(timer) = self.pop_due(now) else {
                break;
            };
            destination.push(timer);
            fired += 1;
        }
        TimerDrain::new(
            fired,
            self.next_deadline()
                .is_some_and(|deadline| deadline.is_elapsed_at(now)),
        )
    }

    /// Returns current limits and retained ownership.
    pub fn snapshot(&self) -> TimerQueueSnapshot {
        TimerQueueSnapshot::new(
            self.owner,
            self.limits,
            self.timers.len(),
            self.retained,
            self.next_deadline(),
            self.next_id,
        )
    }

    fn check_retained(
        &self,
        deadline: Deadline,
        value: T,
        retained: RetainedBytes,
    ) -> Result<Checked<T>, TimerScheduleError<T>> {
        let Some(total) = self.retained.checked_add(retained) else {
            return Err(TimerScheduleError::new(
                deadline,
                value,
                TimerScheduleFailure::RetainedByteOverflow {
                    current: self.retained,
                    timer: retained,
                },
            ));
        };
        if total.get() > self.limits.retained_bytes().get() {
            return Err(TimerScheduleError::new(
                deadline,
                value,
                TimerScheduleFailure::RetainedByteCapacity {
                    limit: self.limits.retained_bytes(),
                    current: self.retained,
                    timer: retained,
                },
            ));
        }
        Ok(Checked { value, total })
    }

    fn subtract_retained(&mut self, removed: RetainedBytes) {
        self.retained = self
            .retained
            .checked_sub(removed)
            .unwrap_or_else(|| panic!("timer retained-byte accounting invariant violated"));
    }
}

#[derive(Debug)]
struct Checked<T> {
    value: T,
    total: RetainedBytes,
}
