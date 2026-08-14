//! Preallocated owner-local slots and bounded resource admission.

use alloc::vec::Vec;
use core::num::NonZeroUsize;

use super::{
    ResourceAdmissionError, ResourceAdmissionFailure, ResourceGeneration, ResourceOwnerId,
    ResourceSlotId, ResourceToken, Slot,
};

/// Bounded owner-local resource slots fenced by monotonic generations.
#[derive(Debug)]
pub struct ResourceTable<K, R> {
    pub(super) owner: ResourceOwnerId,
    pub(super) capacity: NonZeroUsize,
    pub(super) slots: Vec<Slot<K, R>>,
    pub(super) active: usize,
    pub(super) exhausted: usize,
}

impl<K, R> ResourceTable<K, R> {
    /// Creates empty slots beginning at generation zero.
    pub fn new(owner: ResourceOwnerId, capacity: NonZeroUsize) -> Self {
        Self::starting_at(owner, capacity, ResourceGeneration::INITIAL)
    }

    /// Creates empty slots beginning at an explicit generation floor.
    ///
    /// This supports restoring an externally persisted floor and deterministic
    /// exhaustion tests. Never lower the floor while an old token may exist.
    pub fn starting_at(
        owner: ResourceOwnerId,
        capacity: NonZeroUsize,
        generation: ResourceGeneration,
    ) -> Self {
        let mut slots = Vec::with_capacity(capacity.get());
        slots.resize_with(capacity.get(), || Slot::Vacant { generation });
        Self {
            owner,
            capacity,
            slots,
            active: 0,
            exhausted: 0,
        }
    }

    /// Returns the table owner identity.
    pub const fn owner(&self) -> ResourceOwnerId {
        self.owner
    }

    /// Returns the fixed slot count.
    pub const fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }

    /// Returns whether no live resources are admitted.
    pub const fn is_empty(&self) -> bool {
        self.active == 0
    }

    /// Returns the live resource count.
    pub const fn len(&self) -> usize {
        self.active
    }

    /// Returns whether a live resource owns `identity`.
    pub fn contains_identity(&self, identity: &K) -> bool
    where
        K: PartialEq,
    {
        self.slots.iter().any(|slot| {
            matches!(slot, Slot::Occupied { identity: current, .. } if current == identity)
        })
    }

    /// Admits one unique identity and returns its exact generation token.
    pub fn admit(
        &mut self,
        identity: K,
        resource: R,
    ) -> Result<ResourceToken, ResourceAdmissionError<K, R>>
    where
        K: Eq,
    {
        if self.contains_identity(&identity) {
            return Err(ResourceAdmissionError::new(
                identity,
                resource,
                ResourceAdmissionFailure::IdentityInUse,
            ));
        }
        if self.active == self.capacity.get() {
            return Err(ResourceAdmissionError::new(
                identity,
                resource,
                ResourceAdmissionFailure::CapacityReached {
                    limit: self.capacity,
                },
            ));
        }
        let Some((index, generation)) = self.next_vacant() else {
            return Err(ResourceAdmissionError::new(
                identity,
                resource,
                ResourceAdmissionFailure::TokenSpaceExhausted,
            ));
        };

        let slot = slot_id(index);
        self.slots[index] = Slot::Occupied {
            generation,
            identity,
            resource,
        };
        self.active = self
            .active
            .checked_add(1)
            .unwrap_or_else(|| panic!("resource table active count overflowed"));
        Ok(ResourceToken::new(self.owner, slot, generation))
    }

    /// Returns the current token for one exact live identity.
    pub fn token_for(&self, identity: &K) -> Option<ResourceToken>
    where
        K: PartialEq,
    {
        self.slots
            .iter()
            .enumerate()
            .find_map(|(index, slot)| match slot {
                Slot::Occupied {
                    generation,
                    identity: current,
                    ..
                } if current == identity => Some(ResourceToken::new(
                    self.owner,
                    slot_id(index),
                    *generation,
                )),
                Slot::Vacant { .. } | Slot::Occupied { .. } | Slot::Exhausted => None,
            })
    }

    fn next_vacant(&self) -> Option<(usize, ResourceGeneration)> {
        self.slots
            .iter()
            .enumerate()
            .find_map(|(index, slot)| match slot {
                Slot::Vacant { generation } => Some((index, *generation)),
                Slot::Occupied { .. } | Slot::Exhausted => None,
            })
    }
}

fn slot_id(index: usize) -> ResourceSlotId {
    let value = u64::try_from(index)
        .unwrap_or_else(|_| panic!("resource slot exceeds the fixed-width identity domain"));
    ResourceSlotId::new(value)
}
