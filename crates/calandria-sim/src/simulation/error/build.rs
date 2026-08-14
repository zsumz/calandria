//! Static simulation construction failures.

use core::{fmt, num::NonZeroUsize};

use calandria::Moment;

/// Why a deterministic simulation could not be constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationBuildError {
    /// The fixed topology exceeds its configured capacity.
    DutyCapacity {
        /// Configured duty limit.
        limit: NonZeroUsize,
        /// Requested static duty count.
        actual: usize,
    },
    /// Duty and event capacities cannot share one ready-set allocation domain.
    ReadyCapacityOverflow {
        /// Static duty count.
        duties: usize,
        /// Pending-event capacity.
        events: NonZeroUsize,
    },
    /// Initial virtual time is already beyond the run ceiling.
    InitialTimeBeyondLimit {
        /// Requested initial virtual moment.
        initial: Moment,
        /// Configured maximum virtual moment.
        limit: Moment,
    },
}

impl fmt::Display for SimulationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DutyCapacity { limit, actual } => write!(
                formatter,
                "simulation topology has {actual} duties but limit is {limit}"
            ),
            Self::ReadyCapacityOverflow { duties, events } => write!(
                formatter,
                "combining {duties} duties with {events} pending events overflows ready capacity"
            ),
            Self::InitialTimeBeyondLimit { initial, limit } => write!(
                formatter,
                "initial moment {}ns exceeds virtual-time limit {}ns",
                initial.as_nanos(),
                limit.as_nanos()
            ),
        }
    }
}

impl core::error::Error for SimulationBuildError {}
