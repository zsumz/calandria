//! Stable identity and ownership envelopes for scheduled events.

use calandria::Moment;

/// Stable identity for one event timeline.
///
/// Callers must assign distinct values to timelines whose tokens may be mixed.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TimelineId(u64);

impl TimelineId {
    /// Creates a timeline identity.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the fixed-width identity value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable timeline-local identity for one scheduled event.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EventId(u64);

impl EventId {
    pub(crate) const MIN: Self = Self(0);
    pub(crate) const MAX: Self = Self(u64::MAX);

    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric timeline-local identity.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Exact handle for one scheduled event.
///
/// When timelines whose tokens may meet use distinct identities, the timeline
/// field prevents a coincidental local identity and moment match.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EventToken {
    timeline: TimelineId,
    id: EventId,
    at: Moment,
}

impl EventToken {
    pub(crate) const fn new(timeline: TimelineId, id: EventId, at: Moment) -> Self {
        Self { timeline, id, at }
    }

    /// Returns the timeline identity.
    pub const fn timeline(self) -> TimelineId {
        self.timeline
    }

    /// Returns the event identity.
    pub const fn id(self) -> EventId {
        self.id
    }

    /// Returns the scheduled virtual moment.
    pub const fn at(self) -> Moment {
        self.at
    }
}

/// One owned event selected for deterministic delivery.
#[derive(Debug, Eq, PartialEq)]
pub struct Delivery<E> {
    token: EventToken,
    event: E,
}

impl<E> Delivery<E> {
    pub(crate) const fn new(token: EventToken, event: E) -> Self {
        Self { token, event }
    }

    /// Returns the event token.
    pub const fn token(&self) -> EventToken {
        self.token
    }

    /// Returns the delivery moment.
    pub const fn at(&self) -> Moment {
        self.token.at()
    }

    /// Borrows the event value.
    pub const fn event(&self) -> &E {
        &self.event
    }

    /// Consumes the envelope and returns the event value.
    pub fn into_event(self) -> E {
        self.event
    }

    /// Splits the token from the owned event.
    pub fn into_parts(self) -> (EventToken, E) {
        (self.token, self.event)
    }
}
