//! First-canonical-action scheduling for exact scenario tests.

use core::convert::Infallible;

use calandria::Moment;

use crate::{ActionKey, ReadySet, Scheduler};

/// Selects the first canonical enabled action.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fifo;

impl Fifo {
    /// Creates a FIFO scheduler.
    pub const fn new() -> Self {
        Self
    }
}

impl Scheduler for Fifo {
    type Error = Infallible;

    fn choose(&mut self, _now: Moment, ready: ReadySet<'_>) -> Result<ActionKey, Self::Error> {
        match ready.first() {
            Some(action) => Ok(action),
            None => panic!("scheduler requires a nonempty ready set"),
        }
    }
}
