//! Ownership-preserving group ingress failures.

use std::{fmt, num::NonZeroUsize};

use crate::{AdmissionFailure, Lane};

use super::ReactorId;

/// Why a reactor group rejected routed ingress.
#[derive(Debug)]
pub enum ReactorGroupSendFailure {
    /// Group admission was closed before publication.
    Closed,
    /// The requested reactor is outside the static topology.
    UnknownReactor {
        /// Rejected reactor identity.
        reactor: ReactorId,
        /// Static number of reactors in the group.
        reactors: NonZeroUsize,
    },
    /// The selected reactor mailbox rejected publication.
    Mailbox(AdmissionFailure),
}

/// Rejected routed value with exact ownership preserved.
#[derive(Debug)]
pub struct ReactorGroupSendError<T> {
    item: T,
    lane: Lane,
    failure: ReactorGroupSendFailure,
}

impl<T> ReactorGroupSendError<T> {
    pub(super) const fn new(item: T, lane: Lane, failure: ReactorGroupSendFailure) -> Self {
        Self {
            item,
            lane,
            failure,
        }
    }

    /// Returns the rejected lane.
    pub const fn lane(&self) -> Lane {
        self.lane
    }

    /// Returns the rejection reason.
    pub const fn failure(&self) -> &ReactorGroupSendFailure {
        &self.failure
    }

    /// Consumes the error and returns the rejected value.
    pub fn into_item(self) -> T {
        self.item
    }

    /// Consumes the error and returns every component.
    pub fn into_parts(self) -> (T, Lane, ReactorGroupSendFailure) {
        (self.item, self.lane, self.failure)
    }
}

impl fmt::Display for ReactorGroupSendFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => formatter.write_str("reactor group admission is closed"),
            Self::UnknownReactor { reactor, reactors } => write!(
                formatter,
                "reactor {} is outside the static topology of {reactors}",
                reactor.get()
            ),
            Self::Mailbox(source) => {
                write!(formatter, "reactor mailbox rejected ingress: {source}")
            }
        }
    }
}

impl<T> fmt::Display for ReactorGroupSendError<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} in {:?} lane", self.failure, self.lane)
    }
}

impl<T: fmt::Debug> std::error::Error for ReactorGroupSendError<T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.failure {
            ReactorGroupSendFailure::Mailbox(source) => Some(source),
            ReactorGroupSendFailure::Closed | ReactorGroupSendFailure::UnknownReactor { .. } => {
                None
            }
        }
    }
}
