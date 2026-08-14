//! Fail-closed deterministic kernel and hard execution limit failures.

use core::{fmt, num::NonZeroU64};

use calandria::Moment;

use crate::{DutyId, EventToken};

/// Internal ownership or lifecycle contract violation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelFailure {
    /// A pending event names a target outside the static topology.
    UnknownDeliveryTarget {
        /// Exact pending event token.
        token: EventToken,
        /// Unknown target carried by the event.
        target: DutyId,
    },
    /// A pending event targets a permanently stopped owner.
    DeliveryTargetsStopped {
        /// Exact pending event token.
        token: EventToken,
        /// Stopped delivery target.
        target: DutyId,
    },
    /// Selected metadata and retained event ownership disagree.
    DeliveryTargetMismatch {
        /// Exact selected event token.
        token: EventToken,
        /// Target named by the selected action.
        expected: DutyId,
        /// Target owned by the retained event.
        actual: DutyId,
    },
    /// A selected event disappeared before exact delivery.
    TimelineLostEvent(EventToken),
    /// An owner attempted to stop while work still targeted it.
    StoppedWithPending {
        /// Owner that attempted to stop.
        duty: DutyId,
        /// Deliveries still targeting the owner.
        pending: usize,
    },
    /// All owner action identities have been consumed.
    ActionIdsExhausted,
    /// No ready action or strictly later virtual moment was available.
    NoFutureProgress {
        /// Virtual moment at which progress failed.
        now: Moment,
    },
}

impl fmt::Display for KernelFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDeliveryTarget { token, target } => write!(
                formatter,
                "event {} targets unknown duty {}",
                token.id().get(),
                target.get()
            ),
            Self::DeliveryTargetsStopped { token, target } => write!(
                formatter,
                "event {} targets stopped duty {}",
                token.id().get(),
                target.get()
            ),
            Self::DeliveryTargetMismatch {
                token,
                expected,
                actual,
            } => write!(
                formatter,
                "event {} selected for duty {} but owns duty {}",
                token.id().get(),
                expected.get(),
                actual.get()
            ),
            Self::TimelineLostEvent(token) => {
                write!(
                    formatter,
                    "event {} disappeared before delivery",
                    token.id().get()
                )
            }
            Self::StoppedWithPending { duty, pending } => write!(
                formatter,
                "duty {} stopped with {pending} pending deliveries",
                duty.get()
            ),
            Self::ActionIdsExhausted => formatter.write_str("action identities are exhausted"),
            Self::NoFutureProgress { now } => write!(
                formatter,
                "no ready action or future moment exists at {}ns",
                now.as_nanos()
            ),
        }
    }
}

impl core::error::Error for KernelFailure {}

/// Hard deterministic execution limit reached before another action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitFailure {
    /// The total action limit was reached.
    TotalActions {
        /// Configured total action limit.
        limit: NonZeroU64,
    },
    /// Too many actions ran without virtual time advancing.
    ActionsAtMoment {
        /// Virtual moment at which the limit was reached.
        at: Moment,
        /// Configured zero-time action limit.
        limit: NonZeroU64,
    },
    /// The next meaningful moment exceeds the configured ceiling.
    VirtualTime {
        /// Next meaningful virtual moment.
        next: Moment,
        /// Configured maximum virtual moment.
        limit: Moment,
    },
}

impl fmt::Display for LimitFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TotalActions { limit } => {
                write!(formatter, "total action limit of {limit} was reached")
            }
            Self::ActionsAtMoment { at, limit } => write!(
                formatter,
                "action limit of {limit} was reached at {}ns",
                at.as_nanos()
            ),
            Self::VirtualTime { next, limit } => write!(
                formatter,
                "next moment {}ns exceeds virtual-time limit {}ns",
                next.as_nanos(),
                limit.as_nanos()
            ),
        }
    }
}

impl core::error::Error for LimitFailure {}
