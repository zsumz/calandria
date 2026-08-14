//! Exact adapter failures without protocol policy.

use std::{fmt, io, num::NonZeroUsize};

use calandria::{Interest, ResourceToken};

/// Failure from Mio setup, registration, or bounded polling.
#[derive(Debug)]
pub enum MioError {
    /// Mio or the operating system rejected an operation.
    Io(io::Error),
    /// The active registration limit was reached.
    RegistrationCapacity {
        /// Configured registration limit.
        limit: NonZeroUsize,
    },
    /// The exact resource generation is already registered.
    AlreadyRegistered {
        /// Duplicate resource token.
        token: ResourceToken,
    },
    /// The exact resource generation has no active registration.
    NotRegistered {
        /// Unknown resource token.
        token: ResourceToken,
    },
    /// The current target cannot express one or more requested interests.
    UnsupportedInterest {
        /// Rejected nonempty interest set.
        interest: Interest,
    },
    /// Every backend token identity has been consumed.
    TokenSpaceExhausted,
    /// The caller's destination cannot retain one full backend batch.
    DestinationTooSmall {
        /// Required destination capacity.
        required: NonZeroUsize,
        /// Supplied destination capacity.
        actual: NonZeroUsize,
    },
}

impl fmt::Display for MioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(formatter, "Mio operation failed: {source}"),
            Self::RegistrationCapacity { limit } => {
                write!(
                    formatter,
                    "Mio registration capacity of {limit} was reached"
                )
            }
            Self::AlreadyRegistered { token } => {
                write!(formatter, "resource token {token:?} is already registered")
            }
            Self::NotRegistered { token } => {
                write!(formatter, "resource token {token:?} is not registered")
            }
            Self::UnsupportedInterest { interest } => {
                write!(
                    formatter,
                    "Mio cannot express interest {interest:?} on this target"
                )
            }
            Self::TokenSpaceExhausted => {
                formatter.write_str("Mio backend token identities are exhausted")
            }
            Self::DestinationTooSmall { required, actual } => write!(
                formatter,
                "poll destination capacity {actual} is smaller than required {required}"
            ),
        }
    }
}

impl std::error::Error for MioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::RegistrationCapacity { .. }
            | Self::AlreadyRegistered { .. }
            | Self::NotRegistered { .. }
            | Self::UnsupportedInterest { .. }
            | Self::TokenSpaceExhausted
            | Self::DestinationTooSmall { .. } => None,
        }
    }
}

impl From<io::Error> for MioError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}
