//! Single-owner storage for one bounded event batch.

use alloc::{vec, vec::Vec};
use core::{fmt, iter::FusedIterator};

use crate::{Retained, RetainedBytes};

use super::{EventBatchError, EventBatchFailure, EventBatchLimits, EventBatchSnapshot};

/// Reusable count- and retained-byte-bounded owner-local event batch.
#[derive(Debug)]
pub struct EventBatch<E> {
    limits: EventBatchLimits,
    events: Vec<Entry<E>>,
    retained: RetainedBytes,
}

impl<E: Retained> EventBatch<E> {
    /// Allocates an empty batch with fixed logical limits.
    pub fn new(limits: EventBatchLimits) -> Self {
        Self {
            limits,
            events: Vec::with_capacity(limits.events().get()),
            retained: RetainedBytes::ZERO,
        }
    }

    /// Returns whether the batch owns no events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the retained event count.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Borrows retained events in insertion order.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &E> + ExactSizeIterator + '_ {
        self.events.iter().map(|entry| &entry.event)
    }

    /// Borrows the first retained event.
    pub fn first(&self) -> Option<&E> {
        self.events.first().map(|entry| &entry.event)
    }

    /// Admits one event without exceeding count or retained-byte limits.
    pub fn try_push(&mut self, event: E) -> Result<(), EventBatchError<E>> {
        if self.events.len() >= self.limits.events().get() {
            return Err(EventBatchError::new(
                event,
                EventBatchFailure::EventCapacity {
                    limit: self.limits.events(),
                },
            ));
        }

        let retained = event.retained_bytes();
        let Some(next_retained) = self.retained.checked_add(retained) else {
            return Err(EventBatchError::new(
                event,
                EventBatchFailure::RetainedByteOverflow {
                    current: self.retained,
                    event: retained,
                },
            ));
        };
        if next_retained.get() > self.limits.retained_bytes().get() {
            return Err(EventBatchError::new(
                event,
                EventBatchFailure::RetainedByteCapacity {
                    limit: self.limits.retained_bytes(),
                    current: self.retained,
                    event: retained,
                },
            ));
        }

        self.events.push(Entry { event, retained });
        self.retained = next_retained;
        Ok(())
    }

    /// Removes the most recently admitted event and restores its accounting.
    pub fn pop(&mut self) -> Option<E> {
        let entry = self.events.pop()?;
        self.retained = subtract_retained(self.retained, entry.retained);
        Some(entry.event)
    }

    /// Drops every retained event and restores empty accounting.
    pub fn clear(&mut self) {
        self.events.clear();
        self.retained = RetainedBytes::ZERO;
    }

    /// Transfers every event in insertion order and empties the batch.
    pub fn drain(&mut self) -> EventBatchDrain<'_, E> {
        self.retained = RetainedBytes::ZERO;
        EventBatchDrain {
            entries: self.events.drain(..),
        }
    }

    /// Returns current limits and retained ownership.
    pub fn snapshot(&self) -> EventBatchSnapshot {
        EventBatchSnapshot::new(self.limits, self.events.len(), self.retained)
    }
}

#[derive(Debug)]
struct Entry<E> {
    event: E,
    retained: RetainedBytes,
}

/// Owned event iterator returned by [`EventBatch::drain`].
pub struct EventBatchDrain<'a, E> {
    entries: vec::Drain<'a, Entry<E>>,
}

impl<E> fmt::Debug for EventBatchDrain<'_, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EventBatchDrain")
            .field("remaining", &self.entries.len())
            .finish()
    }
}

impl<E> Iterator for EventBatchDrain<'_, E> {
    type Item = E;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().map(|entry| entry.event)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.entries.size_hint()
    }
}

impl<E> DoubleEndedIterator for EventBatchDrain<'_, E> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.entries.next_back().map(|entry| entry.event)
    }
}

impl<E> ExactSizeIterator for EventBatchDrain<'_, E> {}
impl<E> FusedIterator for EventBatchDrain<'_, E> {}

fn subtract_retained(current: RetainedBytes, removed: RetainedBytes) -> RetainedBytes {
    current
        .checked_sub(removed)
        .unwrap_or_else(|| panic!("event-batch retained-byte accounting invariant violated"))
}
