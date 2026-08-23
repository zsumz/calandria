//! Fixed limits and planned waiter failure for singular-reactor tests.

use std::{convert::Infallible, error::Error, fmt, num::NonZeroUsize};

use calandria::{Duty, Moment, Span, Turn, WaitOutcome, Waiter};

#[derive(Debug)]
pub(crate) struct WaitingDuty;

impl Duty for WaitingDuty {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        Ok(Turn::waiting())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WaitFailure;

impl fmt::Display for WaitFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("planned wait failure")
    }
}

impl Error for WaitFailure {}

#[derive(Debug)]
pub(crate) struct FailingWaiter;

impl Waiter<WaitingDuty> for FailingWaiter {
    type Error = WaitFailure;

    fn wait(
        &mut self,
        _duty: &mut WaitingDuty,
        _maximum: Span,
    ) -> Result<WaitOutcome, Self::Error> {
        Err(WaitFailure)
    }
}

pub(crate) fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
