//! Ownership-preserving timer admission failures.

use core::{fmt, num::NonZeroUsize};

use crate::{Deadline, RetainedBytes};

/// Why a timer could not enter a bounded owner-local queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerScheduleFailure {
    /// The pending-timer count reached its hard limit.
    TimerCapacity {
        /// Configured timer-count limit.
        limit: NonZeroUsize,
    },
    /// Retained-byte addition overflowed the fixed-width accounting domain.
    RetainedByteOverflow {
        /// Bytes already retained.
        current: RetainedBytes,
        /// Bytes measured for the rejected timer value.
        timer: RetainedBytes,
    },
    /// The retained-byte limit would be exceeded.
    RetainedByteCapacity {
        /// Configured retained-byte limit.
        limit: RetainedBytes,
        /// Bytes already retained.
        current: RetainedBytes,
        /// Bytes measured for the rejected timer value.
        timer: RetainedBytes,
    },
    /// Every owner-local timer identity has been consumed.
    TimerIdsExhausted,
}

impl fmt::Display for TimerScheduleFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimerCapacity { limit } => {
                write!(formatter, "timer capacity of {limit} was reached")
            }
            Self::RetainedByteOverflow { current, timer } => write!(
                formatter,
                "adding {} retained bytes to {} would overflow timer accounting",
                timer.get(),
                current.get()
            ),
            Self::RetainedByteCapacity {
                limit,
                current,
                timer,
            } => write!(
                formatter,
                "adding {} retained bytes to {} would exceed timer limit {}",
                timer.get(),
                current.get(),
                limit.get()
            ),
            Self::TimerIdsExhausted => formatter.write_str("timer identities are exhausted"),
        }
    }
}

impl core::error::Error for TimerScheduleFailure {}

/// Failed timer admission with the original deadline and value.
#[derive(Debug)]
pub struct TimerScheduleError<T> {
    deadline: Deadline,
    value: T,
    failure: TimerScheduleFailure,
}

impl<T> TimerScheduleError<T> {
    pub(super) const fn new(deadline: Deadline, value: T, failure: TimerScheduleFailure) -> Self {
        Self {
            deadline,
            value,
            failure,
        }
    }

    /// Returns the rejection reason.
    pub const fn failure(&self) -> TimerScheduleFailure {
        self.failure
    }

    /// Returns the rejected absolute deadline.
    pub const fn deadline(&self) -> Deadline {
        self.deadline
    }

    /// Borrows the rejected timer value.
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Returns ownership of the rejected timer value.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Splits the rejected deadline, value, and reason.
    pub fn into_parts(self) -> (Deadline, T, TimerScheduleFailure) {
        (self.deadline, self.value, self.failure)
    }
}

impl<T> fmt::Display for TimerScheduleError<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(formatter)
    }
}

impl<T: fmt::Debug> core::error::Error for TimerScheduleError<T> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        Some(&self.failure)
    }
}
