//! Stable position of one reactor in a static group topology.

/// Stable zero-based identity of one reactor in a group.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReactorId(u64);

impl ReactorId {
    /// Creates an identity from its fixed-width zero-based topology position.
    pub const fn new(position: u64) -> Self {
        Self(position)
    }

    /// Returns the fixed-width zero-based topology position.
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(super) fn from_position(position: usize) -> Self {
        Self(
            u64::try_from(position)
                .unwrap_or_else(|_| panic!("reactor group position exceeds identity domain")),
        )
    }

    pub(super) fn position(self) -> Option<usize> {
        usize::try_from(self.0).ok()
    }
}
