//! Owner, slot, and generation validation for resource tokens.

use super::{ResourceTable, ResourceToken, ResourceTokenFailure, Slot, token_failure};

impl<K, R> ResourceTable<K, R> {
    pub(super) fn validate_owner_and_slot(
        &self,
        token: ResourceToken,
    ) -> Result<usize, ResourceTokenFailure> {
        if token.owner() != self.owner {
            return Err(ResourceTokenFailure::OwnerMismatch {
                expected: self.owner,
                actual: token.owner(),
            });
        }
        let Ok(index) = usize::try_from(token.slot().get()) else {
            return Err(ResourceTokenFailure::SlotOutOfBounds {
                slot: token.slot(),
                capacity: self.capacity,
            });
        };
        if index >= self.capacity.get() {
            return Err(ResourceTokenFailure::SlotOutOfBounds {
                slot: token.slot(),
                capacity: self.capacity,
            });
        }
        Ok(index)
    }

    pub(super) fn validate_token(
        &self,
        token: ResourceToken,
    ) -> Result<usize, ResourceTokenFailure> {
        let index = self.validate_owner_and_slot(token)?;
        match &self.slots[index] {
            Slot::Occupied { generation, .. } if *generation == token.generation() => Ok(index),
            slot => Err(token_failure(slot, token)),
        }
    }
}
