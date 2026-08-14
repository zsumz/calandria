//! Immutable post-action state exposed to deterministic monitors.

use alloc::collections::BTreeMap;

use crate::{DutyId, Model, Topology};

use super::{DutySnapshot, DutyState, SimulationSnapshot};

/// Immutable view of model and kernel state after one committed action.
#[derive(Clone, Copy, Debug)]
pub struct SimulationView<'a, M: Model> {
    model: &'a M,
    topology: &'a Topology,
    states: &'a BTreeMap<DutyId, DutyState>,
    snapshot: SimulationSnapshot,
}

impl<'a, M: Model> SimulationView<'a, M> {
    pub(crate) const fn new(
        model: &'a M,
        topology: &'a Topology,
        states: &'a BTreeMap<DutyId, DutyState>,
        snapshot: SimulationSnapshot,
    ) -> Self {
        Self {
            model,
            topology,
            states,
            snapshot,
        }
    }

    /// Returns immutable consumer-owned model state.
    pub const fn model(self) -> &'a M {
        self.model
    }

    /// Returns the fixed deterministic topology.
    pub const fn topology(self) -> &'a Topology {
        self.topology
    }

    /// Returns one owner's current state.
    pub fn duty(self, duty: DutyId) -> Option<DutySnapshot> {
        self.states
            .get(&duty)
            .copied()
            .map(|state| DutySnapshot::new(duty, state))
    }

    /// Returns bounded global kernel state.
    pub const fn snapshot(self) -> SimulationSnapshot {
        self.snapshot
    }
}
