//! Canonical action identity, readiness, and committed action records.

use alloc::vec::Vec;
use core::cmp::Ordering;

use calandria::{Moment, Turn};

use crate::{DutyId, EventToken};

/// Monotonic identity for one attempted model action.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActionId(u64);

impl ActionId {
    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Returns the fixed-width identity value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One enabled operation for a duty at current virtual time.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ActionKind {
    /// Deliver one exact pending event.
    Delivery(EventToken),
    /// Give the owner one bounded turn.
    Turn,
}

/// Canonical scheduler key for one enabled action.
///
/// Due deliveries preserve timeline order before owner turns. Deliveries use
/// virtual moment, timeline, event identity, then target as a final fence.
/// Turns use duty identity. Schedulers receive this exact total order.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ActionKey {
    duty: DutyId,
    kind: ActionKind,
}

impl ActionKey {
    /// Creates an event-delivery action key.
    pub const fn delivery(duty: DutyId, token: EventToken) -> Self {
        Self {
            duty,
            kind: ActionKind::Delivery(token),
        }
    }

    /// Creates an owner-turn action key.
    pub const fn turn(duty: DutyId) -> Self {
        Self {
            duty,
            kind: ActionKind::Turn,
        }
    }

    /// Returns the action's single mutable owner.
    pub const fn duty(self) -> DutyId {
        self.duty
    }

    /// Returns the enabled operation kind.
    pub const fn kind(self) -> ActionKind {
        self.kind
    }
}

impl Ord for ActionKey {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.kind, other.kind) {
            (ActionKind::Delivery(left), ActionKind::Delivery(right)) => left
                .at()
                .cmp(&right.at())
                .then_with(|| left.timeline().cmp(&right.timeline()))
                .then_with(|| left.id().cmp(&right.id()))
                .then_with(|| self.duty.cmp(&other.duty)),
            (ActionKind::Delivery(_), ActionKind::Turn) => Ordering::Less,
            (ActionKind::Turn, ActionKind::Delivery(_)) => Ordering::Greater,
            (ActionKind::Turn, ActionKind::Turn) => self.duty.cmp(&other.duty),
        }
    }
}

impl PartialOrd for ActionKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Copyable identity and causality for one attempted action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionMeta {
    id: ActionId,
    key: ActionKey,
    at: Moment,
    cause: Option<ActionId>,
}

impl ActionMeta {
    pub(crate) const fn new(
        id: ActionId,
        key: ActionKey,
        at: Moment,
        cause: Option<ActionId>,
    ) -> Self {
        Self { id, key, at, cause }
    }

    /// Returns the action identity.
    pub const fn id(self) -> ActionId {
        self.id
    }

    /// Returns the selected canonical action.
    pub const fn key(self) -> ActionKey {
        self.key
    }

    /// Returns the virtual moment at which the action ran.
    pub const fn at(self) -> Moment {
        self.at
    }

    /// Returns the action that scheduled this delivery, when modeled.
    pub const fn cause(self) -> Option<ActionId> {
        self.cause
    }
}

/// One successfully committed model action.
#[derive(Debug)]
pub struct ActionRecord<O> {
    meta: ActionMeta,
    turn: Turn,
    observations: Vec<O>,
}

impl<O> ActionRecord<O> {
    pub(crate) fn new(meta: ActionMeta, turn: Turn, observations: Vec<O>) -> Self {
        Self {
            meta,
            turn,
            observations,
        }
    }

    /// Returns copyable action identity and causality.
    pub const fn meta(&self) -> ActionMeta {
        self.meta
    }

    /// Returns the complete scheduling interest committed by the owner.
    pub const fn turn(&self) -> Turn {
        self.turn
    }

    /// Returns structured observations in emission order.
    pub fn observations(&self) -> &[O] {
        &self.observations
    }

    /// Consumes the record and returns its observations.
    pub fn into_observations(self) -> Vec<O> {
        self.observations
    }
}

/// Canonically ordered, nonempty actions available to a scheduler.
#[derive(Clone, Copy, Debug)]
pub struct ReadySet<'a> {
    actions: &'a [ActionKey],
}

impl<'a> ReadySet<'a> {
    pub(crate) const fn new(actions: &'a [ActionKey]) -> Self {
        Self { actions }
    }

    /// Returns all enabled actions in canonical order.
    pub const fn actions(self) -> &'a [ActionKey] {
        self.actions
    }

    /// Returns the enabled action count.
    pub const fn len(self) -> usize {
        self.actions.len()
    }

    /// Returns whether no action is enabled.
    pub const fn is_empty(self) -> bool {
        self.actions.is_empty()
    }

    /// Returns the first canonical action.
    pub fn first(self) -> Option<ActionKey> {
        if self.actions.is_empty() {
            None
        } else {
            Some(self.actions[0])
        }
    }

    /// Returns whether the exact action is currently enabled.
    pub fn contains(self, action: ActionKey) -> bool {
        self.actions.binary_search(&action).is_ok()
    }
}
