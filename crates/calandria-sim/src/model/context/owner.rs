//! Owned transactional state for one deterministic action callback.

use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};
use core::{mem, num::NonZeroUsize};

use calandria::{Moment, Retained, RetainedBytes};

use crate::{ActionId, EventToken, Timeline};

use super::super::{DutyId, Routed, Topology};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ActionContextLimits {
    pub(crate) effects: NonZeroUsize,
    pub(crate) effect_bytes: RetainedBytes,
    pub(crate) observations: NonZeroUsize,
    pub(crate) observation_bytes: RetainedBytes,
    pub(crate) max_time: Moment,
}

/// Controlled capability surface for one deterministic model action.
///
/// Successful sends and cancellations are immediately reflected in the private
/// timeline so later effects in the same callback see exact capacity. They are
/// rolled back if the callback returns an error or unwinds before commit.
#[derive(Debug)]
pub struct ActionContext<'a, E: Retained, O: Retained> {
    pub(super) timeline: &'a mut Timeline<Routed<E>>,
    pub(super) topology: &'a Topology,
    pub(super) stopped: &'a BTreeSet<DutyId>,
    pub(super) action: ActionId,
    pub(super) now: Moment,
    pub(super) limits: ActionContextLimits,
    pub(super) effects: usize,
    pub(super) effect_bytes: RetainedBytes,
    pub(super) observations: Vec<O>,
    pub(super) observation_bytes: RetainedBytes,
    pub(super) inserted: BTreeMap<EventToken, RetainedBytes>,
    pub(super) removed: Vec<(EventToken, Routed<E>)>,
    pub(super) committed: bool,
}

impl<'a, E: Retained, O: Retained> ActionContext<'a, E, O> {
    pub(crate) fn new(
        timeline: &'a mut Timeline<Routed<E>>,
        topology: &'a Topology,
        stopped: &'a BTreeSet<DutyId>,
        action: ActionId,
        now: Moment,
        limits: ActionContextLimits,
    ) -> Self {
        Self {
            timeline,
            topology,
            stopped,
            action,
            now,
            limits,
            effects: 0,
            effect_bytes: RetainedBytes::ZERO,
            observations: Vec::new(),
            observation_bytes: RetainedBytes::ZERO,
            inserted: BTreeMap::new(),
            removed: Vec::new(),
            committed: false,
        }
    }

    /// Returns current global virtual time.
    pub const fn now(&self) -> Moment {
        self.now
    }

    /// Returns the action that causally owns newly scheduled events.
    pub const fn action(&self) -> ActionId {
        self.action
    }

    pub(crate) fn pending_for(&self, duty: DutyId) -> usize {
        self.timeline
            .pending()
            .filter(|(_, routed)| routed.target == duty)
            .count()
    }

    pub(crate) fn commit(mut self) -> Vec<O> {
        self.committed = true;
        self.inserted.clear();
        self.removed.clear();
        mem::take(&mut self.observations)
    }
}
