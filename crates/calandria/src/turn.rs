//! Scheduling facts returned by one bounded owner turn.

use crate::{Deadline, Moment, Span};

/// Saturating diagnostic count of work completed during a turn.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkCount(u64);

impl WorkCount {
    /// No completed work.
    pub const ZERO: Self = Self(0);

    /// Creates a work count.
    pub const fn new(work: u64) -> Self {
        Self(work)
    }

    /// Returns the completed work count.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Adds counts without wrapping diagnostic state.
    #[must_use]
    pub const fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }
}

/// When an owner requires another scheduling opportunity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Next {
    /// Another bounded turn is immediately runnable.
    Now,
    /// It is safe to wait indefinitely for an external wake.
    Wake,
    /// Wait for an external wake or the absolute deadline.
    WakeOr(Deadline),
    /// This owner has permanently completed its lifecycle.
    Stop,
}

impl Next {
    /// Combines scheduling interest from independent owners.
    ///
    /// Immediate work wins, deadlines select the earliest moment, and `Stop`
    /// acts as the identity for a host aggregating live owners.
    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Now, _) | (_, Self::Now) => Self::Now,
            (Self::WakeOr(left), Self::WakeOr(right)) => {
                if left.moment().as_nanos() <= right.moment().as_nanos() {
                    Self::WakeOr(left)
                } else {
                    Self::WakeOr(right)
                }
            }
            (Self::WakeOr(deadline), Self::Wake | Self::Stop)
            | (Self::Wake | Self::Stop, Self::WakeOr(deadline)) => Self::WakeOr(deadline),
            (Self::Wake, Self::Wake | Self::Stop) | (Self::Stop, Self::Wake) => Self::Wake,
            (Self::Stop, Self::Stop) => Self::Stop,
        }
    }

    /// Returns a bounded host wait for this scheduling interest.
    ///
    /// `Stop` returns the ceiling because a mixed host may still contain live
    /// owners. A host whose aggregate is `Stop` should terminate instead of
    /// waiting.
    pub const fn bounded_wait(self, now: Moment, ceiling: Span) -> Span {
        match self {
            Self::Now => Span::ZERO,
            Self::Wake | Self::Stop => ceiling,
            Self::WakeOr(deadline) => deadline.remaining_at(now).min(ceiling),
        }
    }
}

/// Work and complete scheduling interest from one bounded turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Turn {
    work: WorkCount,
    next: Next,
}

impl Turn {
    /// Creates a turn result.
    pub const fn new(work: WorkCount, next: Next) -> Self {
        Self { work, next }
    }

    /// Reports no work and a safe external-wake wait.
    pub const fn waiting() -> Self {
        Self::new(WorkCount::ZERO, Next::Wake)
    }

    /// Reports immediately runnable work.
    pub const fn runnable(work: WorkCount) -> Self {
        Self::new(work, Next::Now)
    }

    /// Reports an absolute next deadline.
    pub const fn until(work: WorkCount, deadline: Deadline) -> Self {
        Self::new(work, Next::WakeOr(deadline))
    }

    /// Reports terminal lifecycle completion.
    pub const fn stopped(work: WorkCount) -> Self {
        Self::new(work, Next::Stop)
    }

    /// Returns work completed during this turn.
    pub const fn work(self) -> WorkCount {
        self.work
    }

    /// Returns the owner's complete scheduling interest.
    pub const fn next(self) -> Next {
        self.next
    }

    /// Combines results from independent owners or mechanisms.
    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        Self {
            work: self.work.saturating_add(other.work),
            next: self.next.merge(other.next),
        }
    }
}
