//! Deterministic ownership and delivery for one bounded event timeline.

use alloc::collections::{BTreeMap, btree_map::Entry as MapEntry};

use calandria::{Moment, Retained, RetainedBytes, Span};

use crate::{Delivery, EventId, EventToken, Planned, TimelineId, VirtualClock};

use super::{ScheduleError, ScheduleFailure, TimelineLimits, TimelineSnapshot};

/// Deterministic virtual time plus a bounded ordered event schedule.
#[derive(Debug)]
pub struct Timeline<E> {
    pub(super) id: TimelineId,
    pub(super) clock: VirtualClock,
    pub(super) limits: TimelineLimits,
    pub(super) next_id: Option<u64>,
    pub(super) retained: RetainedBytes,
    pub(super) events: BTreeMap<(Moment, EventId), Entry<E>>,
}

impl<E: Retained> Timeline<E> {
    /// Creates an empty timeline at [`Moment::ORIGIN`].
    pub fn new(id: TimelineId, limits: TimelineLimits) -> Self {
        Self::at(id, Moment::ORIGIN, limits)
    }

    /// Creates an empty timeline at an explicit virtual moment.
    pub fn at(id: TimelineId, now: Moment, limits: TimelineLimits) -> Self {
        Self {
            id,
            clock: VirtualClock::at(now),
            limits,
            next_id: Some(0),
            retained: RetainedBytes::ZERO,
            events: BTreeMap::new(),
        }
    }

    /// Returns this timeline's stable identity.
    pub const fn id(&self) -> TimelineId {
        self.id
    }

    /// Returns current virtual time.
    pub const fn now(&self) -> Moment {
        self.clock.now()
    }

    /// Returns whether no events are retained.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the current pending event count.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns the earliest scheduled moment without changing ownership.
    pub fn next_at(&self) -> Option<Moment> {
        self.events.keys().next().map(|(at, _)| *at)
    }

    /// Schedules an event at an absolute virtual moment.
    pub fn schedule_at(
        &mut self,
        at: Moment,
        event: E,
    ) -> Result<EventToken, ScheduleError<E>> {
        if at < self.now() {
            return Err(ScheduleError::new(
                event,
                ScheduleFailure::ScheduledInPast {
                    current: self.now(),
                    requested: at,
                },
            ));
        }
        self.schedule_validated(at, event)
    }

    /// Schedules an event after a relative virtual-time delay.
    pub fn schedule_after(
        &mut self,
        delay: Span,
        event: E,
    ) -> Result<EventToken, ScheduleError<E>> {
        let Some(at) = self.now().checked_add(delay) else {
            return Err(ScheduleError::new(
                event,
                ScheduleFailure::TimeOverflow {
                    current: self.now(),
                    delay,
                },
            ));
        };
        self.schedule_validated(at, event)
    }

    /// Schedules one delayed deterministic outcome.
    pub fn schedule_planned(
        &mut self,
        planned: Planned<E>,
    ) -> Result<EventToken, ScheduleError<E>> {
        let delay = planned.delay();
        self.schedule_after(delay, planned.into_outcome())
    }

    /// Cancels an event and returns its value when it is still pending.
    pub fn cancel(&mut self, token: EventToken) -> Option<E> {
        if token.timeline() != self.id {
            return None;
        }
        let entry = self.events.remove(&(token.at(), token.id()))?;
        self.retained = subtract_retained(self.retained, entry.retained);
        Some(entry.event)
    }

    /// Advances to and returns exactly one event.
    ///
    /// Equal-time events are delivered in insertion order.
    pub fn pop_next(&mut self) -> Option<Delivery<E>> {
        let ((at, id), entry) = self.events.pop_first()?;
        self.clock
            .advance_to(at)
            .unwrap_or_else(|_| panic!("timeline ordering invariant violated"));
        self.retained = subtract_retained(self.retained, entry.retained);
        Some(Delivery::new(EventToken::new(self.id, id, at), entry.event))
    }

    /// Returns current bounded ownership and virtual-time state.
    pub fn snapshot(&self) -> TimelineSnapshot {
        TimelineSnapshot::new(
            self.id,
            self.limits,
            self.now(),
            self.events.len(),
            self.retained,
            self.next_at(),
        )
    }

    fn schedule_validated(
        &mut self,
        at: Moment,
        event: E,
    ) -> Result<EventToken, ScheduleError<E>> {
        if self.events.len() >= self.limits.pending_events().get() {
            return Err(ScheduleError::new(
                event,
                ScheduleFailure::EventCapacity {
                    limit: self.limits.pending_events(),
                },
            ));
        }

        let retained = event.retained_bytes();
        let Some(next_retained) = self.retained.checked_add(retained) else {
            return Err(ScheduleError::new(
                event,
                ScheduleFailure::RetainedByteOverflow {
                    current: self.retained,
                    event: retained,
                },
            ));
        };
        if next_retained.get() > self.limits.retained_bytes().get() {
            return Err(ScheduleError::new(
                event,
                ScheduleFailure::RetainedByteCapacity {
                    limit: self.limits.retained_bytes(),
                    current: self.retained,
                    event: retained,
                },
            ));
        }

        let Some(raw_id) = self.next_id else {
            return Err(ScheduleError::new(
                event,
                ScheduleFailure::EventIdsExhausted,
            ));
        };
        let id = EventId::from_raw(raw_id);
        self.next_id = raw_id.checked_add(1);
        let token = EventToken::new(self.id, id, at);
        match self.events.entry((at, id)) {
            MapEntry::Vacant(slot) => {
                slot.insert(Entry { event, retained });
            }
            MapEntry::Occupied(_) => panic!("timeline event identity collision"),
        }
        self.retained = next_retained;
        Ok(token)
    }
}

#[derive(Debug)]
pub(super) struct Entry<E> {
    pub(super) event: E,
    pub(super) retained: RetainedBytes,
}

fn subtract_retained(current: RetainedBytes, removed: RetainedBytes) -> RetainedBytes {
    current
        .checked_sub(removed)
        .unwrap_or_else(|| panic!("timeline retained-byte accounting invariant violated"))
}
