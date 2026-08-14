//! Owner interest and immutable deterministic simulation observations.

use calandria::{Moment, Next, RetainedBytes};

use crate::{DutyId, TimelineSnapshot};

/// Lifecycle state retained by the deterministic kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationPhase {
    /// More actions or external injections may be accepted.
    Active,
    /// Every duty stopped and no delivery remains owned.
    Completed,
    /// A model, monitor, scheduler, or kernel failure ended execution.
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DutyState {
    pub(crate) next: Next,
    pub(crate) turns: u64,
    pub(crate) deliveries: u64,
}

impl DutyState {
    pub(crate) const fn initial() -> Self {
        Self {
            next: Next::Now,
            turns: 0,
            deliveries: 0,
        }
    }
}

/// Immutable state for one simulated owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DutySnapshot {
    duty: DutyId,
    next: Next,
    turns: u64,
    deliveries: u64,
}

impl DutySnapshot {
    pub(crate) const fn new(duty: DutyId, state: DutyState) -> Self {
        Self {
            duty,
            next: state.next,
            turns: state.turns,
            deliveries: state.deliveries,
        }
    }

    /// Returns the duty identity.
    pub const fn duty(self) -> DutyId {
        self.duty
    }

    /// Returns the duty's complete current scheduling interest.
    pub const fn next(self) -> Next {
        self.next
    }

    /// Returns successful bounded owner turns.
    pub const fn turns(self) -> u64 {
        self.turns
    }

    /// Returns successful typed event deliveries.
    pub const fn deliveries(self) -> u64 {
        self.deliveries
    }
}

/// Immutable bounded state for one simulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationSnapshot {
    phase: SimulationPhase,
    timeline: TimelineSnapshot,
    duties: usize,
    stopped: usize,
    actions: u64,
    actions_at_moment: u64,
}

impl SimulationSnapshot {
    pub(crate) const fn new(
        phase: SimulationPhase,
        timeline: TimelineSnapshot,
        duties: usize,
        stopped: usize,
        actions: u64,
        actions_at_moment: u64,
    ) -> Self {
        Self {
            phase,
            timeline,
            duties,
            stopped,
            actions,
            actions_at_moment,
        }
    }

    /// Returns current lifecycle state.
    pub const fn phase(self) -> SimulationPhase {
        self.phase
    }

    /// Returns current global virtual time.
    pub const fn now(self) -> Moment {
        self.timeline.now()
    }

    /// Returns the static duty count.
    pub const fn duties(self) -> usize {
        self.duties
    }

    /// Returns the permanently stopped duty count.
    pub const fn stopped(self) -> usize {
        self.stopped
    }

    /// Returns pending modeled delivery count.
    pub const fn pending_events(self) -> usize {
        self.timeline.pending_events()
    }

    /// Returns variable bytes retained by pending deliveries.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.timeline.retained_bytes()
    }

    /// Returns attempted model action count.
    pub const fn actions(self) -> u64 {
        self.actions
    }

    /// Returns actions attempted without advancing virtual time.
    pub const fn actions_at_moment(self) -> u64 {
        self.actions_at_moment
    }

    /// Returns the earliest pending modeled delivery moment.
    pub const fn next_event_at(self) -> Option<Moment> {
        self.timeline.next_at()
    }
}
