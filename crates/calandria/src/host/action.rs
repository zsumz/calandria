//! Pure host decisions derived from complete scheduling interest.

use crate::{Moment, Next, Span, Turn};

use super::HostConfig;

/// Action selected after one bounded duty turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostAction {
    /// Run another bounded turn without waiting.
    Continue,
    /// Request progress observation with the supplied blocking bound.
    Wait(Span),
    /// Terminate this host because its duty permanently stopped.
    Stop,
}

impl HostAction {
    /// Plans the next host action from a complete turn result.
    ///
    /// This pure seam is also usable by explicit domain hosts that preserve
    /// their own concrete owner order instead of implementing [`super::Duty`].
    pub const fn for_turn(turn: Turn, now: Moment, config: HostConfig) -> Self {
        Self::for_next(turn.next(), now, config)
    }

    /// Plans the next host action from scheduling interest.
    pub const fn for_next(next: Next, now: Moment, config: HostConfig) -> Self {
        match next {
            Next::Now => Self::Continue,
            Next::Wake => Self::from_wait(config.wait_ceiling()),
            Next::WakeOr(deadline) => {
                let wait = deadline.remaining_at(now).min(config.wait_ceiling());
                Self::from_wait(wait)
            }
            Next::Stop => Self::Stop,
        }
    }

    /// Returns the selected wait span, if this action waits.
    pub const fn wait_span(self) -> Option<Span> {
        match self {
            Self::Wait(wait) => Some(wait),
            Self::Continue | Self::Stop => None,
        }
    }

    const fn from_wait(wait: Span) -> Self {
        if wait.as_nanos() == 0 {
            Self::Continue
        } else {
            Self::Wait(wait)
        }
    }
}

/// Observation produced by one successful embedded host step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostStep {
    started_at: Moment,
    completed_at: Moment,
    turn: Turn,
    action: HostAction,
}

impl HostStep {
    pub(super) const fn new(
        started_at: Moment,
        completed_at: Moment,
        turn: Turn,
        action: HostAction,
    ) -> Self {
        Self {
            started_at,
            completed_at,
            turn,
            action,
        }
    }

    /// Returns the moment supplied to the duty.
    pub const fn started_at(self) -> Moment {
        self.started_at
    }

    /// Returns the post-turn moment used to calculate waiting.
    pub const fn completed_at(self) -> Moment {
        self.completed_at
    }

    /// Returns the duty's complete bounded-turn result.
    pub const fn turn(self) -> Turn {
        self.turn
    }

    /// Returns the action selected for the next host iteration.
    pub const fn action(self) -> HostAction {
        self.action
    }
}
