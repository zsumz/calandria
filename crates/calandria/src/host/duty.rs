//! One bounded unit of owner-local work.

use crate::{Moment, Turn};

/// A hostable owner that performs one bounded turn at a time.
///
/// A reactor may implement this trait, but the trait does not require I/O,
/// readiness, sockets, or any other reactor-specific mechanism. State
/// machines, compaction loops, worker coordinators, and maintenance owners can
/// use the same host.
///
/// # Contract
///
/// Each call must return after bounded work, must not hide an unbounded wait,
/// and must report the owner's complete current [`Turn`] interest. Returning
/// [`crate::Next::Stop`] permanently ends this duty's hosted lifecycle.
///
/// The supplied [`Moment`] is authoritative for one invocation. Every deadline
/// comparison, timeout decision, behavior-changing state transition or
/// timestamp, and returned [`crate::Next`] must derive from that observation.
/// A duty must not sample a wall or monotonic clock during `turn`: time may
/// advance between invocations, but not within one logical turn.
///
/// After `turn` returns, the host may observe its clock again to account for
/// execution time and calculate the actual wait duration.
pub trait Duty {
    /// Failure returned by one bounded turn.
    type Error;

    /// Performs one bounded turn using the supplied monotonic observation.
    fn turn(&mut self, now: Moment) -> Result<Turn, Self::Error>;
}

impl<F, E> Duty for F
where
    F: FnMut(Moment) -> Result<Turn, E>,
{
    type Error = E;

    fn turn(&mut self, now: Moment) -> Result<Turn, Self::Error> {
        self(now)
    }
}
