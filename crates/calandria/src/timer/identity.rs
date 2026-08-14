//! Stable owner-local timer identity and cancellation token.

use crate::Deadline;

/// Stable identity for one timer owner.
///
/// Callers must assign distinct values to queues whose tokens may be mixed.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TimerOwnerId(u64);

impl TimerOwnerId {
    /// Creates a timer-owner identity.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the fixed-width identity value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Monotonic owner-local identity for one admitted timer.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TimerId(u64);

impl TimerId {
    /// The first identity used by [`crate::TimerQueue::new`].
    pub const ZERO: Self = Self(0);

    /// Creates an identity from its fixed-width value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the fixed-width identity value.
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(super) const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Exact handle for one admitted timer.
///
/// When queues whose tokens may meet use distinct owner identities, the owner
/// field prevents a coincidental local identity and deadline match.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TimerToken {
    owner: TimerOwnerId,
    id: TimerId,
    deadline: Deadline,
}

impl TimerToken {
    pub(super) const fn new(
        owner: TimerOwnerId,
        id: TimerId,
        deadline: Deadline,
    ) -> Self {
        Self {
            owner,
            id,
            deadline,
        }
    }

    /// Returns the timer-owner identity.
    pub const fn owner(self) -> TimerOwnerId {
        self.owner
    }

    /// Returns the timer identity.
    pub const fn id(self) -> TimerId {
        self.id
    }

    /// Returns the absolute deadline captured at admission.
    pub const fn deadline(self) -> Deadline {
        self.deadline
    }
}
