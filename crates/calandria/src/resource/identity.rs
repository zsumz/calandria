//! Explicit owner, slot, and generation identity for resource tokens.

/// Stable identity for one mutable resource owner.
///
/// Callers must assign distinct values to tables whose tokens may be mixed.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceOwnerId(u64);

impl ResourceOwnerId {
    /// Creates an owner identity.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the fixed-width identity value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Fixed-width slot identity inside one resource owner.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceSlotId(u64);

impl ResourceSlotId {
    /// Creates a slot identity from its fixed-width value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the owner-local slot index.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Monotonic reuse generation for one resource slot.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceGeneration(u64);

impl ResourceGeneration {
    /// Initial generation for a new table.
    pub const INITIAL: Self = Self(0);

    /// Last representable generation.
    pub const MAX: Self = Self(u64::MAX);

    /// Creates a generation from its fixed-width value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the fixed-width generation value.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the next generation, or `None` when reuse is exhausted.
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Exact identity for one admitted resource generation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceToken {
    owner: ResourceOwnerId,
    slot: ResourceSlotId,
    generation: ResourceGeneration,
}

impl ResourceToken {
    /// Creates a token from explicit owner, slot, and generation identities.
    ///
    /// Constructing a token does not make it valid. A [`crate::ResourceTable`]
    /// validates every component before exposing or removing a resource.
    pub const fn new(
        owner: ResourceOwnerId,
        slot: ResourceSlotId,
        generation: ResourceGeneration,
    ) -> Self {
        Self {
            owner,
            slot,
            generation,
        }
    }

    /// Returns the resource owner identity.
    pub const fn owner(self) -> ResourceOwnerId {
        self.owner
    }

    /// Returns the owner-local slot identity.
    pub const fn slot(self) -> ResourceSlotId {
        self.slot
    }

    /// Returns the exact slot generation.
    pub const fn generation(self) -> ResourceGeneration {
        self.generation
    }
}
