//! Ownership-preserving event-batch admission failures.

use core::{fmt, num::NonZeroUsize};

use crate::RetainedBytes;

/// Why an event could not enter a bounded owner-local batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventBatchFailure {
    /// The retained event count reached its hard limit.
    EventCapacity {
        /// Configured event-count limit.
        limit: NonZeroUsize,
    },
    /// Retained-byte addition overflowed the fixed-width accounting domain.
    RetainedByteOverflow {
        /// Bytes already retained.
        current: RetainedBytes,
        /// Bytes measured for the rejected event.
        event: RetainedBytes,
    },
    /// The retained-byte limit would be exceeded.
    RetainedByteCapacity {
        /// Configured retained-byte limit.
        limit: RetainedBytes,
        /// Bytes already retained.
        current: RetainedBytes,
        /// Bytes measured for the rejected event.
        event: RetainedBytes,
    },
}

impl fmt::Display for EventBatchFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventCapacity { limit } => {
                write!(formatter, "event batch capacity of {limit} was reached")
            }
            Self::RetainedByteOverflow { current, event } => write!(
                formatter,
                "adding {} retained bytes to {} would overflow event-batch accounting",
                event.get(),
                current.get()
            ),
            Self::RetainedByteCapacity {
                limit,
                current,
                event,
            } => write!(
                formatter,
                "adding {} retained bytes to {} would exceed event-batch limit {}",
                event.get(),
                current.get(),
                limit.get()
            ),
        }
    }
}

impl core::error::Error for EventBatchFailure {}

/// Failed event-batch admission with ownership of the rejected value.
#[derive(Debug)]
pub struct EventBatchError<E> {
    event: E,
    failure: EventBatchFailure,
}

impl<E> EventBatchError<E> {
    pub(super) const fn new(event: E, failure: EventBatchFailure) -> Self {
        Self { event, failure }
    }

    /// Returns the rejection reason.
    pub const fn failure(&self) -> EventBatchFailure {
        self.failure
    }

    /// Returns ownership of the rejected event.
    pub fn into_event(self) -> E {
        self.event
    }

    /// Splits the rejected event from its reason.
    pub fn into_parts(self) -> (E, EventBatchFailure) {
        (self.event, self.failure)
    }
}

impl<E> fmt::Display for EventBatchError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(formatter)
    }
}

impl<E: fmt::Debug> core::error::Error for EventBatchError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        Some(&self.failure)
    }
}
