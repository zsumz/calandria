//! Static simulation construction failures.

use core::{fmt, num::NonZeroUsize};

use calandria::Moment;

use crate::Topology;

/// Why a deterministic simulation could not be constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationBuildFailure {
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

impl fmt::Display for SimulationBuildFailure {
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

impl core::error::Error for SimulationBuildFailure {}

/// Construction failure retaining every consumer-supplied component.
pub struct SimulationBuildError<M, S, N> {
    failure: SimulationBuildFailure,
    model: M,
    topology: Topology,
    scheduler: S,
    monitor: N,
}

impl<M, S, N> SimulationBuildError<M, S, N> {
    pub(in crate::simulation) const fn new(
        failure: SimulationBuildFailure,
        model: M,
        topology: Topology,
        scheduler: S,
        monitor: N,
    ) -> Self {
        Self {
            failure,
            model,
            topology,
            scheduler,
            monitor,
        }
    }

    /// Returns the construction failure boundary.
    pub const fn failure(&self) -> &SimulationBuildFailure {
        &self.failure
    }

    /// Consumes the error and returns its failure and rejected components.
    pub fn into_parts(self) -> (SimulationBuildFailure, M, Topology, S, N) {
        (
            self.failure,
            self.model,
            self.topology,
            self.scheduler,
            self.monitor,
        )
    }
}

impl<M, S, N> fmt::Display for SimulationBuildError<M, S, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(formatter)
    }
}

impl<M, S, N> fmt::Debug for SimulationBuildError<M, S, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SimulationBuildError")
            .field("failure", &self.failure)
            .finish_non_exhaustive()
    }
}

impl<M, S, N> core::error::Error for SimulationBuildError<M, S, N> {}
