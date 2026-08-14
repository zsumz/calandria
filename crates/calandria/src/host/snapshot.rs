//! Saturating host diagnostics that never participate in ownership decisions.

use crate::{Moment, Turn, WorkCount};

use super::HostAction;

/// Lifecycle phase of one bounded host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPhase {
    /// The host may execute another duty turn.
    Running,
    /// The duty returned terminal [`crate::Next::Stop`] interest.
    Stopped,
    /// Clock, duty, or waiting execution failed.
    Failed,
}

/// Point-in-time execution observations for one host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostSnapshot {
    phase: HostPhase,
    turns: u64,
    work: WorkCount,
    last_moment: Option<Moment>,
    last_turn: Option<Turn>,
    last_action: Option<HostAction>,
}

impl HostSnapshot {
    pub(super) const fn new() -> Self {
        Self {
            phase: HostPhase::Running,
            turns: 0,
            work: WorkCount::ZERO,
            last_moment: None,
            last_turn: None,
            last_action: None,
        }
    }

    /// Returns the lifecycle phase.
    pub const fn phase(self) -> HostPhase {
        self.phase
    }

    /// Returns the saturating successful-turn count.
    pub const fn turns(self) -> u64 {
        self.turns
    }

    /// Returns the saturating work count reported by successful turns.
    pub const fn work(self) -> WorkCount {
        self.work
    }

    /// Returns the latest accepted monotonic observation.
    pub const fn last_moment(self) -> Option<Moment> {
        self.last_moment
    }

    /// Returns the most recent successful duty result.
    pub const fn last_turn(self) -> Option<Turn> {
        self.last_turn
    }

    /// Returns the most recent selected host action.
    pub const fn last_action(self) -> Option<HostAction> {
        self.last_action
    }

    pub(super) fn observe(&mut self, moment: Moment) {
        self.last_moment = Some(moment);
    }

    pub(super) fn record(&mut self, turn: Turn, action: HostAction) {
        self.turns = self.turns.saturating_add(1);
        self.work = self.work.saturating_add(turn.work());
        self.last_turn = Some(turn);
        self.last_action = Some(action);
    }

    pub(super) fn stop(&mut self) {
        self.phase = HostPhase::Stopped;
    }

    pub(super) fn fail(&mut self) {
        self.phase = HostPhase::Failed;
    }
}
