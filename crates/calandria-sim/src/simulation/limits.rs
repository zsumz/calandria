//! Hard topology, action, effect, observation, and virtual-time limits.

use core::num::{NonZeroU64, NonZeroUsize};

use calandria::{Moment, RetainedBytes};

use crate::{TimelineLimits, model::ActionContextLimits};

/// Hard limits for one deterministic simulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationLimits {
    max_duties: NonZeroUsize,
    timeline: TimelineLimits,
    effects_per_action: NonZeroUsize,
    effect_bytes_per_action: RetainedBytes,
    observations_per_action: NonZeroUsize,
    observation_bytes_per_action: RetainedBytes,
    total_actions: NonZeroU64,
    actions_per_moment: NonZeroU64,
    max_virtual_time: Moment,
}

impl SimulationLimits {
    /// Creates limits around an event timeline configuration.
    pub const fn new(timeline: TimelineLimits) -> Self {
        Self {
            max_duties: nonzero_usize(256),
            timeline,
            effects_per_action: nonzero_usize(256),
            effect_bytes_per_action: RetainedBytes::new(16 * 1_024 * 1_024),
            observations_per_action: nonzero_usize(256),
            observation_bytes_per_action: RetainedBytes::new(4 * 1_024 * 1_024),
            total_actions: nonzero_u64(1_000_000),
            actions_per_moment: nonzero_u64(100_000),
            max_virtual_time: Moment::from_nanos(u64::MAX),
        }
    }

    /// Replaces the topology capacity.
    pub const fn with_max_duties(mut self, limit: NonZeroUsize) -> Self {
        self.max_duties = limit;
        self
    }

    /// Replaces the successful-effect count for one action.
    pub const fn with_effects_per_action(mut self, limit: NonZeroUsize) -> Self {
        self.effects_per_action = limit;
        self
    }

    /// Replaces the retained-effect byte limit for one action.
    pub const fn with_effect_bytes_per_action(mut self, limit: RetainedBytes) -> Self {
        self.effect_bytes_per_action = limit;
        self
    }

    /// Replaces the observation count for one action.
    pub const fn with_observations_per_action(mut self, limit: NonZeroUsize) -> Self {
        self.observations_per_action = limit;
        self
    }

    /// Replaces the retained-observation byte limit for one action.
    pub const fn with_observation_bytes_per_action(
        mut self,
        limit: RetainedBytes,
    ) -> Self {
        self.observation_bytes_per_action = limit;
        self
    }

    /// Replaces the total committed or failed model-action limit.
    pub const fn with_total_actions(mut self, limit: NonZeroU64) -> Self {
        self.total_actions = limit;
        self
    }

    /// Replaces the zero-time action limit used to detect suspected livelock.
    pub const fn with_actions_per_moment(mut self, limit: NonZeroU64) -> Self {
        self.actions_per_moment = limit;
        self
    }

    /// Replaces the maximum observable virtual moment.
    pub const fn with_max_virtual_time(mut self, limit: Moment) -> Self {
        self.max_virtual_time = limit;
        self
    }

    /// Returns the topology capacity.
    pub const fn max_duties(self) -> NonZeroUsize {
        self.max_duties
    }

    /// Returns the pending-event timeline limits.
    pub const fn timeline(self) -> TimelineLimits {
        self.timeline
    }

    /// Returns the successful-effect count for one action.
    pub const fn effects_per_action(self) -> NonZeroUsize {
        self.effects_per_action
    }

    /// Returns the retained-effect byte limit for one action.
    pub const fn effect_bytes_per_action(self) -> RetainedBytes {
        self.effect_bytes_per_action
    }

    /// Returns the structured-observation count for one action.
    pub const fn observations_per_action(self) -> NonZeroUsize {
        self.observations_per_action
    }

    /// Returns the retained-observation byte limit for one action.
    pub const fn observation_bytes_per_action(self) -> RetainedBytes {
        self.observation_bytes_per_action
    }

    /// Returns the total model-action limit.
    pub const fn total_actions(self) -> NonZeroU64 {
        self.total_actions
    }

    /// Returns the action limit at one virtual moment.
    pub const fn actions_per_moment(self) -> NonZeroU64 {
        self.actions_per_moment
    }

    /// Returns the maximum virtual moment.
    pub const fn max_virtual_time(self) -> Moment {
        self.max_virtual_time
    }

    pub(crate) const fn action_context(self) -> ActionContextLimits {
        ActionContextLimits {
            effects: self.effects_per_action,
            effect_bytes: self.effect_bytes_per_action,
            observations: self.observations_per_action,
            observation_bytes: self.observation_bytes_per_action,
            max_time: self.max_virtual_time,
        }
    }
}

impl Default for SimulationLimits {
    fn default() -> Self {
        Self::new(TimelineLimits::default())
    }
}

const fn nonzero_usize(value: usize) -> NonZeroUsize {
    match NonZeroUsize::new(value) {
        Some(value) => value,
        None => panic!("default simulation count must be nonzero"),
    }
}

const fn nonzero_u64(value: u64) -> NonZeroU64 {
    match NonZeroU64::new(value) {
        Some(value) => value,
        None => panic!("default simulation action count must be nonzero"),
    }
}
