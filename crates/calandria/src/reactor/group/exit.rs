//! Ordered member exits and aggregate reactor-group terminal state.

use std::any::Any;

use crate::{Clock, Duty, ReactorExit, Waiter};

use super::ReactorId;

/// Terminal state of one reactor-group member thread.
pub enum ReactorGroupMemberExit<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    /// The reactor returned an owned terminal exit.
    Exited(ReactorExit<D, C, W>),
    /// The reactor panicked and lost its unwound owner state.
    Panicked(Box<dyn Any + Send + 'static>),
}

/// Aggregate terminal reason for one static reactor group.
///
/// Panics take precedence over typed failures, which take precedence over
/// explicit termination and graceful stop. The lowest reactor identity wins
/// within one fatal class, independent of thread completion order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReactorGroupOutcome {
    /// Every reactor duty stopped without framework termination or failure.
    Stopped,
    /// At least one reactor observed explicit framework termination.
    Terminated,
    /// A reactor returned a typed host or waiting failure.
    Failed(ReactorId),
    /// A reactor thread unwound through a panic.
    Panicked(ReactorId),
}

/// Owned ordered result of one complete group lifecycle.
pub struct ReactorGroupExit<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    outcome: ReactorGroupOutcome,
    members: Box<[ReactorGroupMemberExit<D, C, W>]>,
}

pub(super) struct Supervised<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    pub(super) outcome: ReactorGroupOutcome,
    pub(super) members: Box<[ReactorGroupMemberExit<D, C, W>]>,
}

impl<D, C, W> ReactorGroupExit<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    pub(super) fn new(supervised: Supervised<D, C, W>) -> Self {
        Self {
            outcome: supervised.outcome,
            members: supervised.members,
        }
    }

    /// Returns the aggregate group outcome.
    pub const fn outcome(&self) -> ReactorGroupOutcome {
        self.outcome
    }

    /// Returns the immutable member count.
    pub const fn len(&self) -> usize {
        self.members.len()
    }

    /// Returns whether this exit contains no members.
    pub const fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Returns one member exit by exact reactor identity.
    pub fn member(&self, reactor: ReactorId) -> Option<&ReactorGroupMemberExit<D, C, W>> {
        reactor
            .position()
            .and_then(|position| self.members.get(position))
    }

    /// Iterates terminal members in stable reactor identity order.
    pub fn members(&self) -> impl Iterator<Item = (ReactorId, &ReactorGroupMemberExit<D, C, W>)> {
        self.members
            .iter()
            .enumerate()
            .map(|(index, exit)| (ReactorId::from_position(index), exit))
    }

    /// Consumes the group exit and returns ordered terminal member ownership.
    pub fn into_members(self) -> Box<[ReactorGroupMemberExit<D, C, W>]> {
        self.members
    }
}

impl<D, C, W> ReactorGroupMemberExit<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    /// Returns the owned reactor exit, or the preserved panic payload.
    pub fn into_result(self) -> std::thread::Result<ReactorExit<D, C, W>> {
        match self {
            Self::Exited(exit) => Ok(exit),
            Self::Panicked(payload) => Err(payload),
        }
    }
}

impl<D, C, W> core::fmt::Debug for ReactorGroupMemberExit<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
    ReactorExit<D, C, W>: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Exited(exit) => formatter.debug_tuple("Exited").field(exit).finish(),
            Self::Panicked(_) => formatter.debug_tuple("Panicked").finish_non_exhaustive(),
        }
    }
}

impl<D, C, W> core::fmt::Debug for ReactorGroupExit<D, C, W>
where
    D: Duty,
    D::Error: core::fmt::Debug,
    C: Clock,
    C::Error: core::fmt::Debug,
    W: Waiter<D>,
    W::Error: core::fmt::Debug,
    D: core::fmt::Debug,
    C: core::fmt::Debug,
    W: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ReactorGroupExit")
            .field("outcome", &self.outcome)
            .field("members", &self.members)
            .finish()
    }
}
