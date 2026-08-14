//! Deterministic selection among canonically ordered enabled actions.

mod fifo;
mod round_robin;

pub use fifo::Fifo;
pub use round_robin::RoundRobin;

use crate::{ActionKey, ReadySet};

/// Selects one action from the exact current ready set.
pub trait Scheduler {
    /// Scheduler-internal failure.
    type Error;

    /// Chooses exactly one enabled action.
    fn choose(&mut self, ready: ReadySet<'_>) -> Result<ActionKey, Self::Error>;
}
