//! Owned reactor execution, control, and shared-nothing groups.

mod control;
mod exit;
mod failure;
mod group;
mod handle;
mod owner;
mod runner;
mod snapshot;
mod spawn_error;
#[cfg(all(test, calandria_loom))]
mod sync_test;
mod termination;

pub use exit::ReactorExit;
pub use failure::{ReactorFailure, ReactorOutcome};
pub use group::{
    ReactorGroup, ReactorGroupExit, ReactorGroupHandle, ReactorGroupLimits, ReactorGroupMember,
    ReactorGroupMemberExit, ReactorGroupOutcome, ReactorGroupSendError, ReactorGroupSendFailure,
    ReactorGroupSpawnError, ReactorGroupSpawnFailure, ReactorGroupTermination, ReactorId,
};
pub use handle::ReactorHandle;
pub use owner::Reactor;
pub use snapshot::ReactorSnapshot;
pub use spawn_error::ReactorSpawnError;
pub use termination::{ReactorTermination, ReactorTerminationStatus};

pub(crate) use control::ReactorControl;
