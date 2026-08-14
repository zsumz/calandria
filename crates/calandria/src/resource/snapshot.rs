//! Immutable resource-table ownership observations.

use core::num::NonZeroUsize;

use super::ResourceOwnerId;

/// Current bounded slot state for one resource owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceTableSnapshot {
    owner: ResourceOwnerId,
    capacity: NonZeroUsize,
    active: usize,
    vacant: usize,
    exhausted: usize,
}

impl ResourceTableSnapshot {
    pub(super) const fn new(
        owner: ResourceOwnerId,
        capacity: NonZeroUsize,
        active: usize,
        vacant: usize,
        exhausted: usize,
    ) -> Self {
        Self {
            owner,
            capacity,
            active,
            vacant,
            exhausted,
        }
    }

    /// Returns the table owner identity.
    pub const fn owner(self) -> ResourceOwnerId {
        self.owner
    }

    /// Returns the fixed slot count.
    pub const fn capacity(self) -> NonZeroUsize {
        self.capacity
    }

    /// Returns the live resource count.
    pub const fn active(self) -> usize {
        self.active
    }

    /// Returns slots that may accept another resource.
    pub const fn vacant(self) -> usize {
        self.vacant
    }

    /// Returns permanently retired slots.
    pub const fn exhausted(self) -> usize {
        self.exhausted
    }
}
