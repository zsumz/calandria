//! Exact removal, generation retirement, and slot observations.

use core::mem;

use super::{ResourceTable, ResourceTableSnapshot, ResourceToken, ResourceTokenFailure, Slot};

impl<K, R> ResourceTable<K, R> {
    /// Removes the exact live generation and returns both owned values.
    pub fn remove(&mut self, token: ResourceToken) -> Result<(K, R), ResourceTokenFailure> {
        let index = self.validate_token(token)?;
        let current = mem::replace(&mut self.slots[index], Slot::Exhausted);
        let Slot::Occupied {
            generation,
            identity,
            resource,
        } = current
        else {
            panic!("validated resource slot changed without an owner turn");
        };

        self.active = self
            .active
            .checked_sub(1)
            .unwrap_or_else(|| panic!("resource table active count underflowed"));
        match generation.checked_next() {
            Some(next) => self.slots[index] = Slot::Vacant { generation: next },
            None => {
                self.exhausted = self
                    .exhausted
                    .checked_add(1)
                    .unwrap_or_else(|| panic!("resource exhausted count overflowed"));
            }
        }
        Ok((identity, resource))
    }

    /// Returns current live, vacant, and exhausted slot counts.
    pub fn snapshot(&self) -> ResourceTableSnapshot {
        let unavailable = self
            .active
            .checked_add(self.exhausted)
            .unwrap_or_else(|| panic!("resource slot accounting overflowed"));
        let vacant = self
            .capacity
            .get()
            .checked_sub(unavailable)
            .unwrap_or_else(|| panic!("resource slot accounting diverged"));
        ResourceTableSnapshot::new(
            self.owner,
            self.capacity,
            self.active,
            vacant,
            self.exhausted,
        )
    }
}
