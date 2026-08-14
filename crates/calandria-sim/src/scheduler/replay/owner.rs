//! Replay scheduler ownership and exact validation transitions.

use alloc::{boxed::Box, vec::Vec};

use calandria::{Moment, Turn};

use crate::{ActionKey, ActionMeta, ReadySet, Scheduler, TraceEntry};

use super::{ReplayDivergence, ReplayPosition};

/// Scheduler that replays one exact bounded causal trace.
#[derive(Clone, Debug)]
pub struct Replay {
    entries: Box<[TraceEntry]>,
    cursor: usize,
    position: ReplayPosition,
    awaiting_commit: bool,
}

impl Replay {
    pub(crate) fn from_entries(entries: impl Iterator<Item = TraceEntry>) -> Self {
        Self {
            entries: entries.collect::<Vec<_>>().into_boxed_slice(),
            cursor: 0,
            position: ReplayPosition::MIN,
            awaiting_commit: false,
        }
    }

    /// Returns the next zero-based replay position.
    pub const fn position(&self) -> ReplayPosition {
        self.position
    }

    /// Returns entries not yet validated as committed.
    pub fn remaining(&self) -> usize {
        self.entries.len().saturating_sub(self.cursor)
    }

    /// Returns whether every retained trace entry was replayed exactly.
    pub fn is_complete(&self) -> bool {
        self.remaining() == 0 && !self.awaiting_commit
    }

    fn expected(&self) -> Option<TraceEntry> {
        self.entries.get(self.cursor).copied()
    }
}

impl Scheduler for Replay {
    type Error = ReplayDivergence;

    fn choose(&mut self, now: Moment, ready: ReadySet<'_>) -> Result<ActionKey, Self::Error> {
        if self.awaiting_commit {
            return Err(ReplayDivergence::PendingCommit {
                position: self.position,
            });
        }
        let Some(expected) = self.expected() else {
            return Err(ReplayDivergence::TraceExhausted {
                position: self.position,
                at: now,
                ready: ready.len(),
            });
        };
        if expected.meta().at() != now {
            return Err(ReplayDivergence::Moment {
                position: self.position,
                expected,
                actual: now,
            });
        }
        if !ready.contains(expected.meta().key()) {
            return Err(ReplayDivergence::Unavailable {
                position: self.position,
                expected,
                ready: ready.len(),
            });
        }
        self.awaiting_commit = true;
        Ok(expected.meta().key())
    }

    fn committed(&mut self, action: ActionMeta, turn: Turn) -> Result<(), Self::Error> {
        let actual = TraceEntry::new(action, turn);
        if !self.awaiting_commit {
            return Err(ReplayDivergence::UnexpectedCommit {
                position: self.position,
                actual,
            });
        }
        let expected = self
            .expected()
            .unwrap_or_else(|| panic!("pending replay selection lost its trace entry"));
        if actual != expected {
            return Err(ReplayDivergence::Committed {
                position: self.position,
                expected,
                actual,
            });
        }
        self.cursor = self
            .cursor
            .checked_add(1)
            .unwrap_or_else(|| panic!("replay cursor overflowed local address space"));
        self.position = self
            .position
            .checked_next()
            .unwrap_or_else(|| panic!("replay position exhausted"));
        self.awaiting_commit = false;
        Ok(())
    }

    fn finished(&mut self) -> Result<(), Self::Error> {
        if self.awaiting_commit {
            return Err(ReplayDivergence::PendingCommit {
                position: self.position,
            });
        }
        if self.remaining() != 0 {
            return Err(ReplayDivergence::TraceRemaining {
                position: self.position,
                remaining: self.remaining(),
            });
        }
        Ok(())
    }
}
