//! Ownership-preserving admission and exact stale-token failures.

use core::{fmt, num::NonZeroUsize};

use super::{ResourceGeneration, ResourceOwnerId, ResourceSlotId};

/// Why a resource could not enter a bounded owner-local table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceAdmissionFailure {
    /// Another live resource already owns the supplied identity.
    IdentityInUse,
    /// Every configured slot currently owns a live resource.
    CapacityReached {
        /// Configured slot count.
        limit: NonZeroUsize,
    },
    /// Every non-live slot has exhausted its generation space.
    TokenSpaceExhausted,
}

impl fmt::Display for ResourceAdmissionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityInUse => formatter.write_str("resource identity is already admitted"),
            Self::CapacityReached { limit } => {
                write!(formatter, "resource capacity of {limit} was reached")
            }
            Self::TokenSpaceExhausted => {
                formatter.write_str("resource token generations are exhausted")
            }
        }
    }
}

impl core::error::Error for ResourceAdmissionFailure {}

/// Failed resource admission preserving both supplied values.
#[derive(Debug)]
pub struct ResourceAdmissionError<K, R> {
    identity: K,
    resource: R,
    failure: ResourceAdmissionFailure,
}

impl<K, R> ResourceAdmissionError<K, R> {
    pub(super) const fn new(identity: K, resource: R, failure: ResourceAdmissionFailure) -> Self {
        Self {
            identity,
            resource,
            failure,
        }
    }

    /// Returns the rejection reason.
    pub const fn failure(&self) -> ResourceAdmissionFailure {
        self.failure
    }

    /// Returns ownership of the rejected identity and resource.
    pub fn into_values(self) -> (K, R) {
        (self.identity, self.resource)
    }

    /// Splits both rejected values from their reason.
    pub fn into_parts(self) -> (K, R, ResourceAdmissionFailure) {
        (self.identity, self.resource, self.failure)
    }
}

impl<K, R> fmt::Display for ResourceAdmissionError<K, R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(formatter)
    }
}

impl<K: fmt::Debug, R: fmt::Debug> core::error::Error for ResourceAdmissionError<K, R> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        Some(&self.failure)
    }
}

/// Why a resource token cannot name a current live resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceTokenFailure {
    /// The token belongs to another resource owner.
    OwnerMismatch {
        /// Owner expected by the table.
        expected: ResourceOwnerId,
        /// Owner carried by the token.
        actual: ResourceOwnerId,
    },
    /// The token names a slot outside the table's fixed capacity.
    SlotOutOfBounds {
        /// Rejected slot.
        slot: ResourceSlotId,
        /// Configured slot count.
        capacity: NonZeroUsize,
    },
    /// The slot is vacant at its next admissible generation.
    Vacant {
        /// Vacant slot.
        slot: ResourceSlotId,
        /// Generation that a future admission will use.
        generation: ResourceGeneration,
    },
    /// The token generation does not match the live resource generation.
    GenerationMismatch {
        /// Named slot.
        slot: ResourceSlotId,
        /// Live generation required by the table.
        current: ResourceGeneration,
        /// Stale or forged generation carried by the token.
        supplied: ResourceGeneration,
    },
    /// The named slot can never be reused.
    Exhausted {
        /// Permanently retired slot.
        slot: ResourceSlotId,
    },
}

impl fmt::Display for ResourceTokenFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerMismatch { expected, actual } => write!(
                formatter,
                "resource token owner {} does not match owner {}",
                actual.get(),
                expected.get()
            ),
            Self::SlotOutOfBounds { slot, capacity } => write!(
                formatter,
                "resource token slot {} exceeds capacity {capacity}",
                slot.get()
            ),
            Self::Vacant { slot, generation } => write!(
                formatter,
                "resource slot {} is vacant at generation {}",
                slot.get(),
                generation.get()
            ),
            Self::GenerationMismatch {
                slot,
                current,
                supplied,
            } => write!(
                formatter,
                "resource slot {} is at generation {}, not {}",
                slot.get(),
                current.get(),
                supplied.get()
            ),
            Self::Exhausted { slot } => {
                write!(formatter, "resource slot {} is exhausted", slot.get())
            }
        }
    }
}

impl core::error::Error for ResourceTokenFailure {}
