//! Shared mailbox state protected by the publication and acknowledgement lock.

use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{RetainedBytes, WakeHandle};

use super::{Lane, LaneSnapshot, MailboxLimits, MailboxSnapshot};

pub(super) struct Shared<T> {
    pub(super) limits: MailboxLimits,
    pub(super) state: Mutex<State<T>>,
    pub(super) counters: Counters,
    pub(super) measure: fn(&T) -> RetainedBytes,
    pub(super) wake: WakeHandle,
}

impl<T> Shared<T> {
    pub(super) fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(super) fn snapshot(&self) -> MailboxSnapshot {
        let state = self.lock();
        MailboxSnapshot {
            control: self.lane_snapshot(Lane::Control, &state.control),
            work: self.lane_snapshot(Lane::Work, &state.work),
            live_senders: state.senders,
            receiver_alive: state.receiver_alive,
            closed_rejections: self.counters.closed.load(Ordering::Relaxed),
            wake_failures: self.counters.wake_failures.load(Ordering::Relaxed),
            wake_requested: self.wake.is_requested(),
        }
    }

    fn lane_snapshot(&self, lane: Lane, state: &LaneState<T>) -> LaneSnapshot {
        let counters = self.counters.lane(lane);
        LaneSnapshot {
            limits: self.limits.lane(lane),
            queued_messages: state.queue.len(),
            retained_bytes: state.retained,
            message_rejections: counters.messages.load(Ordering::Relaxed),
            byte_rejections: counters.bytes.load(Ordering::Relaxed),
        }
    }
}

pub(super) struct State<T> {
    pub(super) control: LaneState<T>,
    pub(super) work: LaneState<T>,
    pub(super) receiver_alive: bool,
    pub(super) senders: usize,
}

impl<T> State<T> {
    pub(super) fn new(limits: MailboxLimits) -> Self {
        Self {
            control: LaneState::new(limits.control().messages()),
            work: LaneState::new(limits.work().messages()),
            receiver_alive: true,
            senders: 1,
        }
    }

    pub(super) const fn lane(&self, lane: Lane) -> &LaneState<T> {
        match lane {
            Lane::Control => &self.control,
            Lane::Work => &self.work,
        }
    }

    pub(super) const fn lane_mut(&mut self, lane: Lane) -> &mut LaneState<T> {
        match lane {
            Lane::Control => &mut self.control,
            Lane::Work => &mut self.work,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.control.queue.is_empty() && self.work.queue.is_empty()
    }
}

pub(super) struct LaneState<T> {
    pub(super) queue: VecDeque<Entry<T>>,
    pub(super) retained: RetainedBytes,
}

impl<T> LaneState<T> {
    fn new(capacity: NonZeroUsize) -> Self {
        Self {
            queue: VecDeque::with_capacity(capacity.get()),
            retained: RetainedBytes::ZERO,
        }
    }

    pub(super) fn admit(&mut self, item: T, retained: RetainedBytes, next_bytes: RetainedBytes) {
        self.queue.push_back(Entry { item, retained });
        self.retained = next_bytes;
    }

    pub(super) fn drain_into(&mut self, count: usize, destination: &mut Vec<T>) {
        for _ in 0..count {
            let Some(entry) = self.queue.pop_front() else {
                return;
            };
            self.retained = self
                .retained
                .checked_sub(entry.retained)
                .unwrap_or_else(|| panic!("mailbox retained-byte accounting invariant violated"));
            destination.push(entry.item);
        }
    }

    pub(super) fn drain_all(&mut self, destination: &mut Vec<T>) {
        self.drain_into(self.queue.len(), destination);
    }
}

pub(super) struct Entry<T> {
    item: T,
    retained: RetainedBytes,
}

pub(super) struct Counters {
    control: LaneCounters,
    work: LaneCounters,
    pub(super) closed: AtomicU64,
    pub(super) wake_failures: AtomicU64,
}

impl Counters {
    pub(super) const fn new() -> Self {
        Self {
            control: LaneCounters::new(),
            work: LaneCounters::new(),
            closed: AtomicU64::new(0),
            wake_failures: AtomicU64::new(0),
        }
    }

    pub(super) const fn lane(&self, lane: Lane) -> &LaneCounters {
        match lane {
            Lane::Control => &self.control,
            Lane::Work => &self.work,
        }
    }

    pub(super) fn increment_closed(&self) {
        increment(&self.closed);
    }

    pub(super) fn increment_wake_failures(&self) {
        increment(&self.wake_failures);
    }
}

pub(super) struct LaneCounters {
    pub(super) messages: AtomicU64,
    pub(super) bytes: AtomicU64,
}

impl LaneCounters {
    const fn new() -> Self {
        Self {
            messages: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
        }
    }

    pub(super) fn increment_messages(&self) {
        increment(&self.messages);
    }

    pub(super) fn increment_bytes(&self) {
        increment(&self.bytes);
    }
}

fn increment(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        Some(value.saturating_add(1))
    });
}
