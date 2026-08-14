//! Virtual monotonic time advanced only by the simulation owner.

use core::{convert::Infallible, fmt};

use calandria::{Clock, Moment, Span};

/// A deterministic clock with no relationship to wall time.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct VirtualClock {
    now: Moment,
}

impl VirtualClock {
    /// Creates a clock at [`Moment::ORIGIN`].
    pub const fn new() -> Self {
        Self {
            now: Moment::ORIGIN,
        }
    }

    /// Creates a clock at an explicit virtual moment.
    pub const fn at(now: Moment) -> Self {
        Self { now }
    }

    /// Returns current virtual time.
    pub const fn now(self) -> Moment {
        self.now
    }

    /// Moves time to `requested` without permitting a backward transition.
    pub fn advance_to(&mut self, requested: Moment) -> Result<(), ClockError> {
        if requested < self.now {
            return Err(ClockError::MovesBackward {
                current: self.now,
                requested,
            });
        }
        self.now = requested;
        Ok(())
    }

    /// Advances virtual time by `span` with checked arithmetic.
    pub fn advance_by(&mut self, span: Span) -> Result<(), ClockError> {
        let Some(requested) = self.now.checked_add(span) else {
            return Err(ClockError::Overflow {
                current: self.now,
                span,
            });
        };
        self.now = requested;
        Ok(())
    }
}

impl Clock for VirtualClock {
    type Error = Infallible;

    fn now(&mut self) -> Result<Moment, Self::Error> {
        Ok(self.now)
    }
}

/// Why a requested virtual-clock transition was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockError {
    /// The requested moment precedes current virtual time.
    MovesBackward {
        /// Current virtual time.
        current: Moment,
        /// Rejected earlier moment.
        requested: Moment,
    },
    /// Adding a span exceeded the fixed-width time domain.
    Overflow {
        /// Current virtual time.
        current: Moment,
        /// Rejected span.
        span: Span,
    },
}

impl fmt::Display for ClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MovesBackward { current, requested } => write!(
                formatter,
                "virtual time cannot move from {}ns back to {}ns",
                current.as_nanos(),
                requested.as_nanos()
            ),
            Self::Overflow { current, span } => write!(
                formatter,
                "advancing virtual time from {}ns by {}ns would overflow",
                current.as_nanos(),
                span.as_nanos()
            ),
        }
    }
}

impl core::error::Error for ClockError {}
