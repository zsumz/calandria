//! Ownership of model, topology, virtual time, scheduler, and monitor state.

use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};
use core::cell::Cell;

use calandria::Moment;

use crate::{
    ActionKey, DutyId, Fifo, Model, NoopMonitor, Routed, Scheduler, Timeline, TimelineId, Topology,
};

use super::{
    DutySnapshot, DutyState, Monitor, SimulationBuildError, SimulationBuildFailure,
    SimulationLimits, SimulationPhase, SimulationSnapshot,
};

/// Deterministic execution owner for one static set of bounded duties.
#[derive(Debug)]
pub struct Simulation<M: Model, S = Fifo, N = NoopMonitor> {
    pub(super) model: M,
    pub(super) topology: Topology,
    pub(super) states: BTreeMap<DutyId, DutyState>,
    pub(super) stopped: BTreeSet<DutyId>,
    pub(super) timeline: Timeline<Routed<M::Event>>,
    pub(super) limits: SimulationLimits,
    pub(super) scheduler: S,
    pub(super) monitor: N,
    pub(super) phase: SimulationPhase,
    pub(super) poisoned: Cell<bool>,
    pub(super) next_action: Option<u64>,
    pub(super) actions: u64,
    pub(super) actions_at_moment: u64,
    pub(super) ready: Vec<ActionKey>,
}

impl<M: Model> Simulation<M, Fifo, NoopMonitor> {
    /// Creates a simulation at the virtual origin using FIFO scheduling.
    pub fn new(
        id: TimelineId,
        model: M,
        topology: Topology,
        limits: SimulationLimits,
    ) -> Result<Self, SimulationBuildError<M, Fifo, NoopMonitor>> {
        Self::with_parts(
            id,
            Moment::ORIGIN,
            model,
            topology,
            limits,
            Fifo::new(),
            NoopMonitor::new(),
        )
    }
}

impl<M: Model, S: Scheduler> Simulation<M, S, NoopMonitor> {
    /// Creates a simulation at the virtual origin with an explicit scheduler.
    pub fn with_scheduler(
        id: TimelineId,
        model: M,
        topology: Topology,
        limits: SimulationLimits,
        scheduler: S,
    ) -> Result<Self, SimulationBuildError<M, S, NoopMonitor>> {
        Self::with_parts(
            id,
            Moment::ORIGIN,
            model,
            topology,
            limits,
            scheduler,
            NoopMonitor::new(),
        )
    }
}

impl<M, S, N> Simulation<M, S, N>
where
    M: Model,
    S: Scheduler,
    N: Monitor<M>,
{
    /// Creates a fully configured simulation at an explicit virtual moment.
    pub fn with_parts(
        id: TimelineId,
        now: Moment,
        model: M,
        topology: Topology,
        limits: SimulationLimits,
        scheduler: S,
        monitor: N,
    ) -> Result<Self, SimulationBuildError<M, S, N>> {
        let events = limits.timeline().pending_events();
        let ready_capacity = topology.len().checked_add(events.get());
        let failure = if topology.len() > limits.max_duties().get() {
            Some(SimulationBuildFailure::DutyCapacity {
                limit: limits.max_duties(),
                actual: topology.len(),
            })
        } else if now > limits.max_virtual_time() {
            Some(SimulationBuildFailure::InitialTimeBeyondLimit {
                initial: now,
                limit: limits.max_virtual_time(),
            })
        } else if ready_capacity.is_none() {
            Some(SimulationBuildFailure::ReadyCapacityOverflow {
                duties: topology.len(),
                events,
            })
        } else {
            None
        };
        if let Some(failure) = failure {
            return Err(SimulationBuildError::new(
                failure, model, topology, scheduler, monitor,
            ));
        }
        let ready_capacity = ready_capacity
            .unwrap_or_else(|| panic!("validated simulation ready capacity must fit"));
        let states = topology
            .duties()
            .iter()
            .copied()
            .map(|duty| (duty, DutyState::initial()))
            .collect();
        let ready = Vec::with_capacity(ready_capacity);
        Ok(Self {
            model,
            topology,
            states,
            stopped: BTreeSet::new(),
            timeline: Timeline::at(id, now, limits.timeline()),
            limits,
            scheduler,
            monitor,
            phase: SimulationPhase::Active,
            poisoned: Cell::new(false),
            next_action: Some(0),
            actions: 0,
            actions_at_moment: 0,
            ready,
        })
    }

    /// Replaces the monitor while preserving exact simulation state.
    pub fn with_monitor<P: Monitor<M>>(self, monitor: P) -> Simulation<M, S, P> {
        Simulation {
            model: self.model,
            topology: self.topology,
            states: self.states,
            stopped: self.stopped,
            timeline: self.timeline,
            limits: self.limits,
            scheduler: self.scheduler,
            monitor,
            phase: self.phase,
            poisoned: self.poisoned,
            next_action: self.next_action,
            actions: self.actions,
            actions_at_moment: self.actions_at_moment,
            ready: self.ready,
        }
    }

    /// Returns immutable consumer-owned model state.
    pub const fn model(&self) -> &M {
        &self.model
    }

    /// Returns the fixed deterministic topology.
    pub const fn topology(&self) -> &Topology {
        &self.topology
    }

    /// Returns immutable scheduler policy state.
    pub const fn scheduler(&self) -> &S {
        &self.scheduler
    }

    /// Returns immutable post-commit monitor state.
    pub const fn monitor(&self) -> &N {
        &self.monitor
    }

    /// Returns current global virtual time.
    pub const fn now(&self) -> Moment {
        self.timeline.now()
    }

    /// Returns current lifecycle state.
    pub const fn phase(&self) -> SimulationPhase {
        self.phase
    }

    /// Returns whether panic unwinding crossed a consumer boundary.
    pub fn is_poisoned(&self) -> bool {
        self.poisoned.get()
    }

    /// Returns one owner's current scheduling and action counts.
    pub fn duty(&self, duty: DutyId) -> Option<DutySnapshot> {
        self.states
            .get(&duty)
            .copied()
            .map(|state| DutySnapshot::new(duty, state))
    }

    /// Consumes the simulation and returns consumer-owned model state.
    pub fn into_model(self) -> M {
        self.model
    }

    /// Consumes the simulation and returns every consumer-supplied component.
    pub fn into_components(self) -> (M, Topology, S, N) {
        (self.model, self.topology, self.scheduler, self.monitor)
    }

    /// Returns bounded deterministic kernel state.
    pub fn snapshot(&self) -> SimulationSnapshot {
        SimulationSnapshot::new(
            self.phase,
            self.timeline.snapshot(),
            self.topology.len(),
            self.stopped.len(),
            self.actions,
            self.actions_at_moment,
        )
    }

    pub(super) fn fail(&mut self) {
        self.phase = SimulationPhase::Failed;
    }
}
