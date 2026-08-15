//! Static shared-nothing reactor topology with bounded typed ingress.

mod admission;
mod exit;
mod gate;
mod identity;
mod limits;
mod member;
mod owner;
mod port;
mod send_error;
mod spawn;
mod spawn_error;
mod supervisor;
mod termination;
mod worker;

pub use exit::{ReactorGroupExit, ReactorGroupMemberExit, ReactorGroupOutcome};
pub use identity::ReactorId;
pub use limits::ReactorGroupLimits;
pub use member::ReactorGroupMember;
pub use owner::ReactorGroup;
pub use port::ReactorGroupHandle;
pub use send_error::{ReactorGroupSendError, ReactorGroupSendFailure};
pub use spawn_error::{ReactorGroupSpawnError, ReactorGroupSpawnFailure};
pub use termination::ReactorGroupTermination;

use exit::Supervised;
use gate::{StartDecision, StartGate};
use port::{AdmissionCloser, bounded_ingress};
use supervisor::{SupervisorAssets, spawn_supervisor};
use termination::GroupControl;
use worker::{ReactorSlot, WorkerJoin, WorkerResult, reactor_slot, spawn_worker};
