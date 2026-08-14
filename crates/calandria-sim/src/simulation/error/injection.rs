//! Ownership-preserving external event injection failures.

use core::fmt;

use calandria::{Moment, Span};

use crate::{DutyId, ScheduleFailure, SimulationPhase};

/// Why an external modeled event could not enter a simulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InjectionFailure {
    /// The simulation was poisoned by panic unwinding.
    Poisoned,
    /// The simulation no longer accepts external work.
    Inactive(SimulationPhase),
    /// The target is absent from the static topology.
    UnknownTarget(DutyId),
    /// The target has permanently stopped.
    TargetStopped(DutyId),
    /// Relative virtual-time arithmetic overflowed.
    TimeOverflow { current: Moment, delay: Span },
    /// The requested moment exceeds the configured ceiling.
    BeyondTimeLimit { requested: Moment, limit: Moment },
    /// The bounded event timeline rejected admission.
    Timeline(ScheduleFailure),
}

impl fmt::Display for InjectionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Poisoned => formatter.write_str("simulation is poisoned"),
            Self::Inactive(phase) => write!(formatter, "simulation is {phase:?}"),
            Self::UnknownTarget(duty) => write!(formatter, "unknown duty {}", duty.get()),
            Self::TargetStopped(duty) => write!(formatter, "duty {} has stopped", duty.get()),
            Self::TimeOverflow { current, delay } => write!(
                formatter,
                "scheduling {}ns after {}ns would overflow virtual time",
                delay.as_nanos(),
                current.as_nanos()
            ),
            Self::BeyondTimeLimit { requested, limit } => write!(
                formatter,
                "event at {}ns exceeds virtual-time limit {}ns",
                requested.as_nanos(),
                limit.as_nanos()
            ),
            Self::Timeline(failure) => failure.fmt(formatter),
        }
    }
}

impl core::error::Error for InjectionFailure {}

/// Ownership-preserving external injection error.
#[derive(Debug)]
pub struct InjectionError<E> {
    event: E,
    failure: InjectionFailure,
}

impl<E> InjectionError<E> {
    pub(crate) const fn new(event: E, failure: InjectionFailure) -> Self {
        Self { event, failure }
    }

    /// Returns the rejection reason.
    pub const fn failure(&self) -> InjectionFailure {
        self.failure
    }

    /// Returns ownership of the rejected event.
    pub fn into_event(self) -> E {
        self.event
    }

    /// Splits the rejected event from its reason.
    pub fn into_parts(self) -> (E, InjectionFailure) {
        (self.event, self.failure)
    }
}

impl<E> fmt::Display for InjectionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(formatter)
    }
}

impl<E: fmt::Debug> core::error::Error for InjectionError<E> {}
