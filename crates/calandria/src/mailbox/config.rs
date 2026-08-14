//! Mailbox lane identities and hard admission limits.

use std::num::NonZeroUsize;

use crate::RetainedBytes;

/// Selects one independently bounded mailbox lane.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Lane {
    /// Lifecycle and control-plane commands drained before ordinary work.
    Control,
    /// Ordinary data-plane work.
    Work,
}

/// Count and retained-byte limits for one mailbox lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaneLimits {
    messages: NonZeroUsize,
    retained_bytes: RetainedBytes,
}

impl LaneLimits {
    /// Creates one lane's hard admission limits.
    ///
    /// A zero retained-byte limit accepts only values measured as retaining no
    /// variable memory. Fixed queue overhead remains bounded by `messages`.
    pub const fn new(messages: NonZeroUsize, retained_bytes: RetainedBytes) -> Self {
        Self {
            messages,
            retained_bytes,
        }
    }

    /// Returns the maximum queued message count.
    pub const fn messages(self) -> NonZeroUsize {
        self.messages
    }

    /// Returns the maximum variable retained bytes.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.retained_bytes
    }
}

/// Independent limits for control and work lanes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MailboxLimits {
    control: LaneLimits,
    work: LaneLimits,
}

impl MailboxLimits {
    /// Creates mailbox limits.
    pub const fn new(control: LaneLimits, work: LaneLimits) -> Self {
        Self { control, work }
    }

    /// Returns the selected lane's limits.
    pub const fn lane(self, lane: Lane) -> LaneLimits {
        match lane {
            Lane::Control => self.control,
            Lane::Work => self.work,
        }
    }

    /// Returns control-lane limits.
    pub const fn control(self) -> LaneLimits {
        self.control
    }

    /// Returns work-lane limits.
    pub const fn work(self) -> LaneLimits {
        self.work
    }
}
