//! Exact immutable and mutable resource-token resolution.

use super::{ResourceTable, ResourceToken, ResourceTokenFailure, Slot, token_failure};

impl<K, R> ResourceTable<K, R> {
    /// Borrows the exact live identity and resource named by `token`.
    pub fn get(&self, token: ResourceToken) -> Result<(&K, &R), ResourceTokenFailure> {
        let index = self.validate_owner_and_slot(token)?;
        match &self.slots[index] {
            Slot::Occupied {
                generation,
                identity,
                resource,
            } if *generation == token.generation() => Ok((identity, resource)),
            slot => Err(token_failure(slot, token)),
        }
    }

    /// Mutably borrows the exact live resource named by `token`.
    pub fn get_mut(&mut self, token: ResourceToken) -> Result<(&K, &mut R), ResourceTokenFailure> {
        let index = self.validate_token(token)?;
        match &mut self.slots[index] {
            Slot::Occupied {
                identity, resource, ..
            } => Ok((identity, resource)),
            slot => Err(token_failure(slot, token)),
        }
    }
}
