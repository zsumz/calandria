//! Exact replay divergence identities and diagnostics.

use core::fmt;

use calandria::Moment;

use crate::TraceEntry;

/// Fixed-width position in one replay trace.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReplayPosition(u64);

impl ReplayPosition {
    pub(super) const MIN: Self = Self(0);

    /// Returns the zero-based trace position.
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(super) const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(next) => Some(Self(next)),
            None => None,
        }
    }
}

/// Exact boundary where a replay departed from its causal trace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayDivergence {
    /// The world exposed another ready set after the trace ended.
    TraceExhausted {
        /// First position absent from the trace.
        position: ReplayPosition,
        /// Actual virtual moment.
        at: Moment,
        /// Actual ready action count.
        ready: usize,
    },
    /// The next traced action became schedulable at a different moment.
    Moment {
        /// Position being replayed.
        position: ReplayPosition,
        /// Exact traced action facts.
        expected: TraceEntry,
        /// Actual virtual moment.
        actual: Moment,
    },
    /// The next traced action was absent from the actual ready set.
    Unavailable {
        /// Position being replayed.
        position: ReplayPosition,
        /// Exact traced action facts.
        expected: TraceEntry,
        /// Actual ready action count.
        ready: usize,
    },
    /// Committed identity, causality, or scheduling interest differed.
    Committed {
        /// Position being replayed.
        position: ReplayPosition,
        /// Exact traced action facts.
        expected: TraceEntry,
        /// Actual committed action facts.
        actual: TraceEntry,
    },
    /// Clean completion occurred before every trace entry was consumed.
    TraceRemaining {
        /// First unconsumed position.
        position: ReplayPosition,
        /// Unconsumed trace entry count.
        remaining: usize,
    },
    /// Selection was requested twice without an intervening commit.
    PendingCommit {
        /// Position awaiting commit validation.
        position: ReplayPosition,
    },
    /// Commit validation was requested without a selected trace entry.
    UnexpectedCommit {
        /// Position that had no selection.
        position: ReplayPosition,
        /// Unexpected committed action facts.
        actual: TraceEntry,
    },
}

impl ReplayDivergence {
    /// Returns the zero-based replay position that diverged.
    pub const fn position(self) -> ReplayPosition {
        match self {
            Self::TraceExhausted { position, .. }
            | Self::Moment { position, .. }
            | Self::Unavailable { position, .. }
            | Self::Committed { position, .. }
            | Self::TraceRemaining { position, .. }
            | Self::PendingCommit { position }
            | Self::UnexpectedCommit { position, .. } => position,
        }
    }
}

impl fmt::Display for ReplayDivergence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let position = self.position().get();
        match self {
            Self::TraceExhausted { at, ready, .. } => write!(
                formatter,
                "trace ended at {position}, but {ready} actions were ready at {}ns",
                at.as_nanos()
            ),
            Self::Moment {
                expected, actual, ..
            } => write!(
                formatter,
                "position {position} expected {}ns but reached {}ns",
                expected.meta().at().as_nanos(),
                actual.as_nanos()
            ),
            Self::Unavailable {
                expected, ready, ..
            } => write!(
                formatter,
                "position {position} expected duty {} absent from {ready} ready actions",
                expected.meta().key().duty().get()
            ),
            Self::Committed { .. } => {
                write!(formatter, "position {position} committed different facts")
            }
            Self::TraceRemaining { remaining, .. } => write!(
                formatter,
                "completion at {position} left {remaining} trace entries"
            ),
            Self::PendingCommit { .. } => {
                write!(formatter, "position {position} still awaits its commit")
            }
            Self::UnexpectedCommit { .. } => {
                write!(
                    formatter,
                    "position {position} received an unselected commit"
                )
            }
        }
    }
}

impl core::error::Error for ReplayDivergence {}
