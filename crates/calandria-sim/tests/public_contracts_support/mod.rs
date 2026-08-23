//! Fixed limits and one planned diagnostic source for public-contract tests.

use core::{
    fmt,
    num::{NonZeroU64, NonZeroUsize},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlannedFailure;

impl fmt::Display for PlannedFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("planned failure")
    }
}

impl core::error::Error for PlannedFailure {}

pub(crate) fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}

pub(crate) fn nonzero_u64(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
