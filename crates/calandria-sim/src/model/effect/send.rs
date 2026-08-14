//! Ownership-preserving event publication failures.

use core::{fmt, num::NonZeroUsize};

use calandria::{Moment, RetainedBytes, Span};

use crate::ScheduleFailure;

use crate::model::DutyId;

/// Why a modeled delivery could not be staged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendFailure {
    /// The action reached its hard successful-effect count.
    EffectCapacity {
        /// Configured effect-count limit.
        limit: NonZeroUsize,
    },
    /// Retained effect accounting overflowed.
    RetainedByteOverflow {
        /// Bytes already retained by staged effects.
        current: RetainedBytes,
        /// Bytes retained by the rejected event.
        event: RetainedBytes,
    },
    /// The action's retained-effect limit would be exceeded.
    RetainedByteCapacity {
        /// Configured retained-effect byte limit.
        limit: RetainedBytes,
        /// Bytes already retained by staged effects.
        current: RetainedBytes,
        /// Bytes retained by the rejected event.
        event: RetainedBytes,
    },
    /// The target is not part of the static topology.
    UnknownTarget(DutyId),
    /// The target has permanently stopped.
    TargetStopped(DutyId),
    /// A requested relative delay overflowed virtual time.
    TimeOverflow {
        /// Current virtual moment.
        current: Moment,
        /// Requested relative delay.
        delay: Span,
    },
    /// The event exceeds the simulation's virtual-time ceiling.
    BeyondTimeLimit {
        /// Requested delivery moment.
        requested: Moment,
        /// Configured maximum virtual moment.
        limit: Moment,
    },
    /// The bounded timeline rejected the event.
    Timeline(ScheduleFailure),
}

impl fmt::Display for SendFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EffectCapacity { limit } => {
                write!(formatter, "action effect capacity of {limit} was reached")
            }
            Self::RetainedByteOverflow { current, event } => write!(
                formatter,
                "adding {} retained effect bytes to {} would overflow",
                event.get(),
                current.get()
            ),
            Self::RetainedByteCapacity {
                limit,
                current,
                event,
            } => write!(
                formatter,
                "adding {} retained effect bytes to {} would exceed {}",
                event.get(),
                current.get(),
                limit.get()
            ),
            Self::UnknownTarget(duty) => write!(formatter, "unknown duty {}", duty.get()),
            Self::TargetStopped(duty) => write!(formatter, "duty {} has stopped", duty.get()),
            Self::TimeOverflow { current, delay } => write!(
                formatter,
                "scheduling {}ns after {}ns would overflow virtual time",
                delay.as_nanos(),
                current.as_nanos()
            ),
            Self::BeyondTimeLimit { requested, limit } => write!(
                formatter,
                "event at {}ns exceeds virtual-time limit {}ns",
                requested.as_nanos(),
                limit.as_nanos()
            ),
            Self::Timeline(failure) => failure.fmt(formatter),
        }
    }
}

impl core::error::Error for SendFailure {}

/// Ownership-preserving modeled-delivery failure.
#[derive(Debug)]
pub struct SendError<E> {
    event: E,
    failure: SendFailure,
}

impl<E> SendError<E> {
    pub(crate) const fn new(event: E, failure: SendFailure) -> Self {
        Self { event, failure }
    }

    /// Returns the rejection reason.
    pub const fn failure(&self) -> SendFailure {
        self.failure
    }

    /// Returns ownership of the rejected event.
    pub fn into_event(self) -> E {
        self.event
    }

    /// Splits the rejected value from its reason.
    pub fn into_parts(self) -> (E, SendFailure) {
        (self.event, self.failure)
    }
}

impl<E> fmt::Display for SendError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(formatter)
    }
}

impl<E: fmt::Debug> core::error::Error for SendError<E> {}
