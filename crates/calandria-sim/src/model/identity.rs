//! Stable identities for explicitly owned simulated duties.

/// Stable identity for one simulated duty.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DutyId(u32);

impl DutyId {
    /// Creates a duty identity.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the fixed-width identity value.
    pub const fn get(self) -> u32 {
        self.0
    }
}
