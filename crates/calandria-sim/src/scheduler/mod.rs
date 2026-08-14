//! Deterministic selection among canonically ordered enabled actions.

mod fifo;
mod replay;
mod round_robin;
mod seeded;

pub use fifo::Fifo;
pub use replay::{Replay, ReplayDivergence, ReplayPosition};
pub use round_robin::RoundRobin;
pub use seeded::{Seeded, SeededError};

use calandria::{Moment, Turn};

use crate::{ActionKey, ActionMeta, ReadySet};

/// Selects one action from the exact current ready set.
pub trait Scheduler {
    /// Scheduler-internal failure.
    type Error;

    /// Chooses exactly one enabled action before model code runs.
    fn choose(&mut self, now: Moment, ready: ReadySet<'_>) -> Result<ActionKey, Self::Error>;

    /// Validates one successfully committed action before monitoring.
    ///
    /// Failure is terminal and cannot roll the committed domain action back.
    fn committed(&mut self, _action: ActionMeta, _turn: Turn) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Validates clean completion after every duty stops and no event remains.
    ///
    /// Resumable quiescence does not invoke this hook.
    fn finished(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
