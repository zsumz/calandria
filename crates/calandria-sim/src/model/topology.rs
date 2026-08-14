//! Fixed deterministic ownership topology for one simulation.

use alloc::vec::Vec;
use core::fmt;

use super::DutyId;

/// Sorted, duplicate-free identities for one static simulation topology.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Topology {
    duties: Vec<DutyId>,
}

impl Topology {
    /// Creates a nonempty static topology.
    pub fn new(
        duties: impl IntoIterator<Item = DutyId>,
    ) -> Result<Self, TopologyError> {
        let mut duties: Vec<_> = duties.into_iter().collect();
        duties.sort_unstable();
        if duties.is_empty() {
            return Err(TopologyError::Empty);
        }
        for pair in duties.windows(2) {
            if pair[0] == pair[1] {
                return Err(TopologyError::Duplicate(pair[0]));
            }
        }
        Ok(Self { duties })
    }

    /// Returns the number of independently owned duties.
    pub fn len(&self) -> usize {
        self.duties.len()
    }

    /// Returns whether the topology contains no duties.
    pub fn is_empty(&self) -> bool {
        self.duties.is_empty()
    }

    /// Returns whether the identity belongs to this topology.
    pub fn contains(&self, duty: DutyId) -> bool {
        self.duties.binary_search(&duty).is_ok()
    }

    /// Returns the canonical sorted duty identities.
    pub fn duties(&self) -> &[DutyId] {
        &self.duties
    }
}

/// Why a static topology could not be constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyError {
    /// At least one duty is required.
    Empty,
    /// A duty identity appeared more than once.
    Duplicate(DutyId),
}

impl fmt::Display for TopologyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("simulation topology must contain a duty"),
            Self::Duplicate(duty) => {
                write!(formatter, "duty {} appears more than once", duty.get())
            }
        }
    }
}

impl core::error::Error for TopologyError {}
