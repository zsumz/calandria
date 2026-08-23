//! Planned scheduler failure used across consumer-boundary scenarios.

use core::fmt;

use calandria_sim::{DutyId, Topology};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlannedFailure;

impl fmt::Display for PlannedFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("planned scheduler failure")
    }
}

impl core::error::Error for PlannedFailure {}

pub(crate) fn topology() -> Topology {
    Topology::new([DutyId::new(1)])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}
