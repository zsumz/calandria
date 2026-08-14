//! Monotonic observations supplied to bounded duties.

use crate::Moment;

/// Source of relative monotonic time for a host.
///
/// Implementations may be production clocks, deterministic test clocks, or
/// embedding-specific clocks. The host independently rejects regression.
pub trait Clock {
    /// Failure to observe the clock.
    type Error;

    /// Returns the current relative monotonic moment.
    fn now(&mut self) -> Result<Moment, Self::Error>;
}

impl<C> Clock for &mut C
where
    C: Clock + ?Sized,
{
    type Error = C::Error;

    fn now(&mut self) -> Result<Moment, Self::Error> {
        (**self).now()
    }
}

#[cfg(feature = "std")]
use std::{sync::Arc, time::Instant};

#[cfg(feature = "std")]
use crate::{DurationOverflow, Span};

/// Production clock measured from one explicit monotonic origin.
///
/// Cloning preserves the origin, so moments and deadlines may move between
/// owners using clones from one clock domain. Independently constructed clocks
/// define independent domains whose moments must not be compared.
#[cfg(feature = "std")]
#[derive(Clone, Debug)]
pub struct MonotonicClock {
    origin: Arc<Instant>,
}

#[cfg(feature = "std")]
impl MonotonicClock {
    /// Creates a clock whose relative origin is the current instant.
    pub fn new() -> Self {
        Self {
            origin: Arc::new(Instant::now()),
        }
    }

    /// Returns whether two clocks share one comparable moment domain.
    pub fn shares_origin(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.origin, &other.origin)
    }
}

#[cfg(feature = "std")]
impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl Clock for MonotonicClock {
    type Error = DurationOverflow;

    fn now(&mut self) -> Result<Moment, Self::Error> {
        let elapsed = Span::try_from(self.origin.elapsed())?;
        Ok(Moment::from_nanos(elapsed.as_nanos()))
    }
}
