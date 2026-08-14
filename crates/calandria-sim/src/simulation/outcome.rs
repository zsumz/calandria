//! One-transition and whole-run deterministic execution outcomes.

use core::fmt;

use calandria::Moment;

use crate::ActionRecord;

use super::SimulationSnapshot;

/// One deterministic kernel transition.
#[derive(Debug)]
pub enum Step<O> {
    /// One model action committed successfully.
    Action(ActionRecord<O>),
    /// No action was ready, so virtual time advanced exactly once.
    TimeAdvanced {
        /// Virtual moment before advancement.
        from: Moment,
        /// Earliest next meaningful virtual moment.
        to: Moment,
    },
    /// Live owners are waiting with no scheduled future progress.
    Quiescent(SimulationSnapshot),
    /// Every owner stopped and no delivery remains owned.
    Completed(SimulationSnapshot),
}

/// Normal reason a run loop returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunEnd {
    /// Every owner stopped and no delivery remains owned.
    Completed,
    /// Live owners are waiting with no scheduled future progress.
    Quiescent,
}

/// Final bounded state from a successful run loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunReport {
    end: RunEnd,
    snapshot: SimulationSnapshot,
}

impl RunReport {
    pub(crate) const fn new(end: RunEnd, snapshot: SimulationSnapshot) -> Self {
        Self { end, snapshot }
    }

    /// Returns why execution returned normally.
    pub const fn end(self) -> RunEnd {
        self.end
    }

    /// Returns final bounded kernel state.
    pub const fn snapshot(self) -> SimulationSnapshot {
        self.snapshot
    }
}

/// Failure from a run loop that expected clean completion.
#[derive(Debug)]
pub enum RunError<E> {
    /// One deterministic transition failed.
    Step(E),
    /// The system became quiescent before all owners stopped.
    Deadlock(SimulationSnapshot),
}

impl<E: fmt::Display> fmt::Display for RunError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Step(error) => error.fmt(formatter),
            Self::Deadlock(snapshot) => write!(
                formatter,
                "simulation deadlocked at {}ns with {} live duties",
                snapshot.now().as_nanos(),
                snapshot.duties().saturating_sub(snapshot.stopped())
            ),
        }
    }
}

impl<E> core::error::Error for RunError<E> where E: fmt::Debug + fmt::Display {}
