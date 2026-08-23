//! Planned clock and duty errors for embedded-host tests.

use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ClockExhausted;

impl fmt::Display for ClockExhausted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("script clock exhausted")
    }
}

impl Error for ClockExhausted {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DutyFailure;

impl fmt::Display for DutyFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("planned duty failure")
    }
}

impl Error for DutyFailure {}
