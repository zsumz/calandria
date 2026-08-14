//! Exact cancellation failure reasons.

use core::{fmt, num::NonZeroUsize};

use calandria::RetainedBytes;

use crate::{EventToken, TimelineId};

/// Why an exact pending-event cancellation was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelFailure {
    /// The action reached its hard successful-effect count.
    EffectCapacity {
        /// Configured effect-count limit.
        limit: NonZeroUsize,
    },
    /// The token belongs to another event timeline.
    ForeignTimeline {
        /// Timeline owned by the action.
        expected: TimelineId,
        /// Timeline carried by the rejected token.
        actual: TimelineId,
    },
    /// The event is no longer pending.
    NotPending(EventToken),
    /// Retaining the removed value for rollback overflowed accounting.
    RetainedByteOverflow {
        /// Bytes already retained for rollback.
        current: RetainedBytes,
        /// Bytes retained by the canceled event.
        event: RetainedBytes,
    },
    /// Retaining the removed value for rollback would exceed the action limit.
    RetainedByteCapacity {
        /// Configured retained-effect byte limit.
        limit: RetainedBytes,
        /// Bytes already retained for rollback.
        current: RetainedBytes,
        /// Bytes retained by the canceled event.
        event: RetainedBytes,
    },
}

impl fmt::Display for CancelFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EffectCapacity { limit } => {
                write!(formatter, "action effect capacity of {limit} was reached")
            }
            Self::ForeignTimeline { expected, actual } => write!(
                formatter,
                "event timeline {} does not match {}",
                actual.get(),
                expected.get()
            ),
            Self::NotPending(token) => {
                write!(formatter, "event {} is not pending", token.id().get())
            }
            Self::RetainedByteOverflow { current, event } => write!(
                formatter,
                "retaining {} canceled bytes beside {} would overflow",
                event.get(),
                current.get()
            ),
            Self::RetainedByteCapacity {
                limit,
                current,
                event,
            } => write!(
                formatter,
                "retaining {} canceled bytes beside {} would exceed {}",
                event.get(),
                current.get(),
                limit.get()
            ),
        }
    }
}

impl core::error::Error for CancelFailure {}
