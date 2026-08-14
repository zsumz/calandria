//! Ownership-preserving mailbox admission failures.

use std::{fmt, io};

use super::Lane;

/// Why mailbox admission rejected an owned value.
#[derive(Debug)]
pub enum AdmissionFailure {
    /// The selected lane reached its message-count limit.
    MessageCapacity,
    /// The selected lane reached or overflowed its retained-byte limit.
    ByteCapacity,
    /// The receiver has closed admission.
    Closed,
    /// The reactor could not be woken, so publication did not occur.
    Wake(io::Error),
}

impl AdmissionFailure {
    /// Returns the backend wake error when wake publication failed.
    pub fn wake_error(&self) -> Option<&io::Error> {
        match self {
            Self::Wake(error) => Some(error),
            Self::MessageCapacity | Self::ByteCapacity | Self::Closed => None,
        }
    }
}

impl fmt::Display for AdmissionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MessageCapacity => formatter.write_str("mailbox message capacity reached"),
            Self::ByteCapacity => formatter.write_str("mailbox retained-byte capacity reached"),
            Self::Closed => formatter.write_str("mailbox receiver is closed"),
            Self::Wake(error) => write!(formatter, "mailbox wake failed: {error}"),
        }
    }
}

impl std::error::Error for AdmissionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.wake_error()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

/// Ownership-preserving mailbox admission error.
#[derive(Debug)]
pub struct TrySendError<T> {
    item: T,
    lane: Lane,
    failure: AdmissionFailure,
}

impl<T> TrySendError<T> {
    pub(super) const fn new(item: T, lane: Lane, failure: AdmissionFailure) -> Self {
        Self {
            item,
            lane,
            failure,
        }
    }

    /// Returns the rejected lane.
    pub const fn lane(&self) -> Lane {
        self.lane
    }

    /// Returns the rejection reason.
    pub const fn failure(&self) -> &AdmissionFailure {
        &self.failure
    }

    /// Returns ownership of the rejected value.
    pub fn into_item(self) -> T {
        self.item
    }

    /// Splits the rejected value from its lane and reason.
    pub fn into_parts(self) -> (T, Lane, AdmissionFailure) {
        (self.item, self.lane, self.failure)
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} in {:?} lane", self.failure, self.lane)
    }
}

impl<T: fmt::Debug> std::error::Error for TrySendError<T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}
