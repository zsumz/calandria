//! Fixed-capacity reusable storage for one readiness batch.

use alloc::{vec, vec::Vec};
use core::{fmt, iter::FusedIterator, num::NonZeroUsize};

use super::PollEvent;

/// Reusable count-bounded readiness destination.
#[derive(Debug)]
pub struct PollEvents {
    capacity: NonZeroUsize,
    events: Vec<PollEvent>,
}

impl PollEvents {
    /// Allocates an empty batch with fixed logical capacity.
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            events: Vec::with_capacity(capacity.get()),
        }
    }

    /// Returns the hard event-count capacity.
    pub const fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }

    /// Returns the retained event count.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns whether no events are retained.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Borrows all observations in backend order.
    pub fn as_slice(&self) -> &[PollEvent] {
        &self.events
    }

    /// Borrows events in backend observation order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &PollEvent> + '_ {
        self.events.iter()
    }

    /// Returns the observation at `index`, if retained.
    pub fn get(&self, index: usize) -> Option<&PollEvent> {
        self.events.get(index)
    }

    /// Clears all observations while retaining allocated storage.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Adds one observation without exceeding the hard capacity.
    pub fn try_push(&mut self, event: PollEvent) -> Result<(), PollEventsError> {
        if self.events.len() >= self.capacity.get() {
            return Err(PollEventsError {
                event,
                capacity: self.capacity,
            });
        }
        self.events.push(event);
        Ok(())
    }

    /// Drains all observations in backend order.
    pub fn drain(&mut self) -> PollEventsDrain<'_> {
        PollEventsDrain {
            events: self.events.drain(..),
        }
    }
}

/// Capacity rejection preserving the readiness observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PollEventsError {
    event: PollEvent,
    capacity: NonZeroUsize,
}

impl PollEventsError {
    /// Returns the rejected observation.
    pub const fn event(self) -> PollEvent {
        self.event
    }

    /// Returns the configured event capacity.
    pub const fn capacity(self) -> NonZeroUsize {
        self.capacity
    }
}

impl fmt::Display for PollEventsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "readiness batch capacity of {} was reached",
            self.capacity
        )
    }
}

impl core::error::Error for PollEventsError {}

/// Owned readiness iterator returned by [`PollEvents::drain`].
pub struct PollEventsDrain<'a> {
    events: vec::Drain<'a, PollEvent>,
}

impl fmt::Debug for PollEventsDrain<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PollEventsDrain")
            .field("remaining", &self.events.len())
            .finish()
    }
}

impl Iterator for PollEventsDrain<'_> {
    type Item = PollEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.events.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.events.size_hint()
    }
}

impl ExactSizeIterator for PollEventsDrain<'_> {}
impl FusedIterator for PollEventsDrain<'_> {}

impl<'a> IntoIterator for &'a PollEvents {
    type Item = &'a PollEvent;
    type IntoIter = core::slice::Iter<'a, PollEvent>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.iter()
    }
}
