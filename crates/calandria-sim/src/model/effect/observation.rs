//! Ownership-preserving structured observation failures.

use core::{fmt, num::NonZeroUsize};

use calandria::RetainedBytes;

/// Why a structured observation could not be retained for one action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationFailure {
    /// The action reached its observation-count limit.
    Capacity { limit: NonZeroUsize },
    /// Observation retained-byte accounting overflowed.
    RetainedByteOverflow {
        current: RetainedBytes,
        observation: RetainedBytes,
    },
    /// The action's retained-observation limit would be exceeded.
    RetainedByteCapacity {
        limit: RetainedBytes,
        current: RetainedBytes,
        observation: RetainedBytes,
    },
}

impl fmt::Display for ObservationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity { limit } => {
                write!(formatter, "action observation capacity of {limit} was reached")
            }
            Self::RetainedByteOverflow {
                current,
                observation,
            } => write!(
                formatter,
                "adding {} observation bytes to {} would overflow",
                observation.get(),
                current.get()
            ),
            Self::RetainedByteCapacity {
                limit,
                current,
                observation,
            } => write!(
                formatter,
                "adding {} observation bytes to {} would exceed {}",
                observation.get(),
                current.get(),
                limit.get()
            ),
        }
    }
}

impl core::error::Error for ObservationFailure {}

/// Ownership-preserving structured-observation failure.
#[derive(Debug)]
pub struct ObservationError<O> {
    observation: O,
    failure: ObservationFailure,
}

impl<O> ObservationError<O> {
    pub(crate) const fn new(observation: O, failure: ObservationFailure) -> Self {
        Self {
            observation,
            failure,
        }
    }

    /// Returns the rejection reason.
    pub const fn failure(&self) -> ObservationFailure {
        self.failure
    }

    /// Returns ownership of the rejected observation.
    pub fn into_observation(self) -> O {
        self.observation
    }
}

impl<O> fmt::Display for ObservationError<O> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(formatter)
    }
}

impl<O: fmt::Debug> core::error::Error for ObservationError<O> {}
