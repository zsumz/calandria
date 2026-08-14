//! Ownership-preserving event scheduling failures.

use core::{fmt, num::NonZeroUsize};

use calandria::{Moment, RetainedBytes, Span};

/// Why one event could not enter a timeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduleFailure {
    /// The requested moment precedes current virtual time.
    ScheduledInPast {
        /// Current virtual time.
        current: Moment,
        /// Rejected earlier moment.
        requested: Moment,
    },
    /// A relative delay overflowed the fixed-width time domain.
    TimeOverflow {
        /// Current virtual time.
        current: Moment,
        /// Rejected delay.
        delay: Span,
    },
    /// The pending-event count reached its hard limit.
    EventCapacity {
        /// Configured event-count limit.
        limit: NonZeroUsize,
    },
    /// Retained-byte addition overflowed its fixed-width accounting domain.
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
    /// Every simulator-local event identity has been consumed.
    EventIdsExhausted,
}

impl fmt::Display for ScheduleFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScheduledInPast { current, requested } => write!(
                formatter,
                "cannot schedule at {}ns before current virtual time {}ns",
                requested.as_nanos(),
                current.as_nanos()
            ),
            Self::TimeOverflow { current, delay } => write!(
                formatter,
                "scheduling {}ns after {}ns would overflow virtual time",
                delay.as_nanos(),
                current.as_nanos()
            ),
            Self::EventCapacity { limit } => {
                write!(formatter, "pending event capacity of {limit} was reached")
            }
            Self::RetainedByteOverflow { current, event } => write!(
                formatter,
                "adding {} retained bytes to {} would overflow accounting",
                event.get(),
                current.get()
            ),
            Self::RetainedByteCapacity {
                limit,
                current,
                event,
            } => write!(
                formatter,
                "adding {} retained bytes to {} would exceed limit {}",
                event.get(),
                current.get(),
                limit.get()
            ),
            Self::EventIdsExhausted => formatter.write_str("event identities are exhausted"),
        }
    }
}

impl core::error::Error for ScheduleFailure {}

/// Ownership-preserving timeline scheduling error.
#[derive(Debug)]
pub struct ScheduleError<E> {
    event: E,
    failure: ScheduleFailure,
}

impl<E> ScheduleError<E> {
    pub(super) const fn new(event: E, failure: ScheduleFailure) -> Self {
        Self { event, failure }
    }

    /// Returns the rejection reason.
    pub const fn failure(&self) -> ScheduleFailure {
        self.failure
    }

    /// Returns ownership of the rejected event.
    pub fn into_event(self) -> E {
        self.event
    }

    /// Splits the rejected event from its reason.
    pub fn into_parts(self) -> (E, ScheduleFailure) {
        (self.event, self.failure)
    }
}

impl<E> fmt::Display for ScheduleError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(formatter)
    }
}

impl<E: fmt::Debug> core::error::Error for ScheduleError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        Some(&self.failure)
    }
}
