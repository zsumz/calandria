//! Stable owner-level fairness without randomized scheduling.

use core::convert::Infallible;

use calandria::Moment;

use crate::{ActionKey, DutyId, ReadySet, Scheduler};

/// Rotates across owners while preserving canonical order within one owner.
#[derive(Clone, Copy, Debug, Default)]
pub struct RoundRobin {
    cursor: Option<DutyId>,
}

impl RoundRobin {
    /// Creates a round-robin scheduler before the first owner.
    pub const fn new() -> Self {
        Self { cursor: None }
    }

    /// Returns the owner selected most recently.
    pub const fn cursor(self) -> Option<DutyId> {
        self.cursor
    }
}

impl Scheduler for RoundRobin {
    type Error = Infallible;

    fn choose(&mut self, _now: Moment, ready: ReadySet<'_>) -> Result<ActionKey, Self::Error> {
        let actions = ready.actions();
        let after = self.cursor.and_then(|cursor| {
            actions
                .iter()
                .map(|action| action.duty())
                .filter(|duty| *duty > cursor)
                .min()
        });
        let owner = after.or_else(|| actions.iter().map(|action| action.duty()).min());
        let Some(owner) = owner else {
            panic!("scheduler requires a nonempty ready set");
        };
        let action = actions
            .iter()
            .find(|action| action.duty() == owner)
            .copied()
            .unwrap_or_else(|| panic!("selected owner must retain a ready action"));
        self.cursor = Some(owner);
        Ok(action)
    }
}
