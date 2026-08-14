//! Thread-owned execution of one bounded duty.

mod exit;
mod failure;
mod handle;
mod runner;
mod snapshot;

pub use exit::DedicatedExit;
pub use failure::{DedicatedFailure, DedicatedOutcome};
pub use handle::DedicatedHost;
pub use snapshot::DedicatedSnapshot;
