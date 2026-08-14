//! Fixed-width monotonic time shared by production and simulation hosts.

use core::{fmt, time::Duration};

/// Nanoseconds elapsed from one owner-defined monotonic origin.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Moment(u64);

impl Moment {
    /// The relative monotonic origin.
    pub const ORIGIN: Self = Self(0);

    /// Creates a moment from elapsed nanoseconds.
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    /// Returns elapsed nanoseconds from the owner-defined origin.
    pub const fn as_nanos(self) -> u64 {
        self.0
    }

    /// Advances by `span`, returning `None` if the time domain overflows.
    pub const fn checked_add(self, span: Span) -> Option<Self> {
        match self.0.checked_add(span.0) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Measures elapsed time since `earlier`, or `None` if it is later.
    pub const fn duration_since(self, earlier: Self) -> Option<Span> {
        match self.0.checked_sub(earlier.0) {
            Some(value) => Some(Span(value)),
            None => None,
        }
    }
}

/// A nonnegative virtual or monotonic duration in nanoseconds.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Span(u64);

impl Span {
    /// A zero-length span.
    pub const ZERO: Self = Self(0);

    /// Creates a span from nanoseconds.
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    /// Creates a span from milliseconds with checked multiplication.
    pub const fn checked_from_millis(milliseconds: u64) -> Option<Self> {
        match milliseconds.checked_mul(1_000_000) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the span as nanoseconds.
    pub const fn as_nanos(self) -> u64 {
        self.0
    }

    /// Returns the span as a core duration.
    pub const fn as_duration(self) -> Duration {
        Duration::from_nanos(self.0)
    }

    /// Adds two spans, returning `None` on overflow.
    pub const fn checked_add(self, other: Self) -> Option<Self> {
        match self.0.checked_add(other.0) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the smaller span.
    #[must_use]
    pub const fn min(self, other: Self) -> Self {
        if self.0 <= other.0 { self } else { other }
    }
}

impl TryFrom<Duration> for Span {
    type Error = DurationOverflow;

    fn try_from(duration: Duration) -> Result<Self, Self::Error> {
        let nanos =
            u64::try_from(duration.as_nanos()).map_err(|_| DurationOverflow { duration })?;
        Ok(Self(nanos))
    }
}

impl From<Span> for Duration {
    fn from(span: Span) -> Self {
        span.as_duration()
    }
}

/// An absolute moment by which an owner must receive another turn.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Deadline(Moment);

impl Deadline {
    /// Creates an absolute deadline.
    pub const fn at(moment: Moment) -> Self {
        Self(moment)
    }

    /// Returns the absolute moment.
    pub const fn moment(self) -> Moment {
        self.0
    }

    /// Returns whether this deadline is due at `now`.
    pub const fn is_elapsed_at(self, now: Moment) -> bool {
        self.0.0 <= now.0
    }

    /// Returns the remaining span, clamped to zero once elapsed.
    pub const fn remaining_at(self, now: Moment) -> Span {
        match self.0.0.checked_sub(now.0) {
            Some(value) => Span(value),
            None => Span::ZERO,
        }
    }
}

impl From<Moment> for Deadline {
    fn from(moment: Moment) -> Self {
        Self::at(moment)
    }
}

/// A core duration could not fit in Calandria's fixed-width time domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurationOverflow {
    duration: Duration,
}

impl DurationOverflow {
    /// Returns the rejected duration.
    pub const fn duration(self) -> Duration {
        self.duration
    }
}

impl fmt::Display for DurationOverflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "duration {:?} exceeds the u64 nanosecond time domain",
            self.duration
        )
    }
}

impl core::error::Error for DurationOverflow {}
