//! Ownership-preserving reactor-group validation and thread creation failures.

use std::{fmt, io, num::NonZeroUsize};

use super::{ReactorGroupMember, ReactorId};

/// Exact boundary that rejected reactor-group startup.
#[derive(Debug)]
pub enum ReactorGroupSpawnFailure {
    /// A static group must contain at least one reactor.
    Empty,
    /// The supplied topology exceeded its hard reactor count limit.
    Capacity {
        /// Configured maximum reactor count.
        limit: NonZeroUsize,
        /// Supplied member count.
        actual: usize,
    },
    /// One reactor owner thread could not be created.
    ReactorThread {
        /// Reactor whose thread creation failed.
        reactor: ReactorId,
        /// Operating-system thread creation failure.
        source: io::Error,
    },
    /// The bounded group supervisor thread could not be created.
    SupervisorThread {
        /// Operating-system thread creation failure.
        source: io::Error,
    },
}

/// Failed group startup with every unstarted member returned in topology order.
pub struct ReactorGroupSpawnError<D, C, W, T> {
    failure: ReactorGroupSpawnFailure,
    members: Vec<ReactorGroupMember<D, C, W, T>>,
}

impl<D, C, W, T> ReactorGroupSpawnError<D, C, W, T> {
    pub(super) const fn new(
        failure: ReactorGroupSpawnFailure,
        members: Vec<ReactorGroupMember<D, C, W, T>>,
    ) -> Self {
        Self { failure, members }
    }

    /// Returns the startup failure boundary.
    pub const fn failure(&self) -> &ReactorGroupSpawnFailure {
        &self.failure
    }

    /// Consumes the error and returns every unstarted member.
    pub fn into_members(self) -> Vec<ReactorGroupMember<D, C, W, T>> {
        self.members
    }

    /// Consumes the error and returns its failure and all members.
    pub fn into_parts(
        self,
    ) -> (
        ReactorGroupSpawnFailure,
        Vec<ReactorGroupMember<D, C, W, T>>,
    ) {
        (self.failure, self.members)
    }
}

impl fmt::Display for ReactorGroupSpawnFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("reactor group topology is empty"),
            Self::Capacity { limit, actual } => write!(
                formatter,
                "reactor group has {actual} members but limit is {limit}"
            ),
            Self::ReactorThread { reactor, source } => write!(
                formatter,
                "reactor {} thread creation failed: {source}",
                reactor.get()
            ),
            Self::SupervisorThread { source } => {
                write!(
                    formatter,
                    "reactor group supervisor creation failed: {source}"
                )
            }
        }
    }
}

impl<D, C, W, T> fmt::Display for ReactorGroupSpawnError<D, C, W, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(formatter)
    }
}

impl<D, C, W, T> fmt::Debug for ReactorGroupSpawnError<D, C, W, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReactorGroupSpawnError")
            .field("failure", &self.failure)
            .field("member_count", &self.members.len())
            .finish_non_exhaustive()
    }
}

impl<D, C, W, T> std::error::Error for ReactorGroupSpawnError<D, C, W, T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.failure {
            ReactorGroupSpawnFailure::ReactorThread { source, .. }
            | ReactorGroupSpawnFailure::SupervisorThread { source } => Some(source),
            ReactorGroupSpawnFailure::Empty | ReactorGroupSpawnFailure::Capacity { .. } => None,
        }
    }
}
