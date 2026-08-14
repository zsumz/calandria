//! Owned timer envelope preserving admission-time measurement.

use crate::{Deadline, Retained, RetainedBytes};

use super::TimerToken;

/// One owned timer removed by cancellation or due delivery.
#[derive(Debug, Eq, PartialEq)]
pub struct Timer<T> {
    token: TimerToken,
    value: T,
    retained: RetainedBytes,
}

impl<T> Timer<T> {
    pub(super) const fn new(token: TimerToken, value: T, retained: RetainedBytes) -> Self {
        Self {
            token,
            value,
            retained,
        }
    }

    /// Returns the exact queue-local token.
    pub const fn token(&self) -> TimerToken {
        self.token
    }

    /// Returns the absolute deadline.
    pub const fn deadline(&self) -> Deadline {
        self.token.deadline()
    }

    /// Borrows the timer value.
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Returns the admission-time retained-byte measurement.
    pub const fn measured_retained_bytes(&self) -> RetainedBytes {
        self.retained
    }

    /// Consumes the timer and returns its value.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Splits the exact token from the owned value.
    pub fn into_parts(self) -> (TimerToken, T) {
        (self.token, self.value)
    }
}

impl<T> Retained for Timer<T> {
    fn retained_bytes(&self) -> RetainedBytes {
        self.retained
    }
}
