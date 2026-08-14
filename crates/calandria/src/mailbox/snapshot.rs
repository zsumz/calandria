//! Mailbox drain results and pressure observations.

use crate::RetainedBytes;

use super::{Lane, LaneLimits, MailboxLimits};

/// Result of one bounded drain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrainReport {
    pub(super) drained: usize,
    pub(super) status: DrainStatus,
}

impl DrainReport {
    /// Returns the number of values transferred into the destination.
    pub const fn drained(self) -> usize {
        self.drained
    }

    /// Returns mailbox scheduling state after the drain.
    pub const fn status(self) -> DrainStatus {
        self.status
    }
}

/// Scheduling state after one bounded mailbox drain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrainStatus {
    /// No queued work remains, but at least one sender is alive.
    Idle,
    /// Queued work remains and another bounded drain is immediately required.
    MorePending,
    /// No queued work remains and no future admission can succeed.
    Closed,
}

/// Current and cumulative state for one lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaneSnapshot {
    pub(super) limits: LaneLimits,
    pub(super) queued_messages: usize,
    pub(super) retained_bytes: RetainedBytes,
    pub(super) message_rejections: u64,
    pub(super) byte_rejections: u64,
}

impl LaneSnapshot {
    /// Returns configured limits.
    pub const fn limits(self) -> LaneLimits {
        self.limits
    }

    /// Returns the current queued value count.
    pub const fn queued_messages(self) -> usize {
        self.queued_messages
    }

    /// Returns current variable retained bytes.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.retained_bytes
    }

    /// Returns cumulative count-capacity rejections.
    pub const fn message_rejections(self) -> u64 {
        self.message_rejections
    }

    /// Returns cumulative byte-capacity rejections.
    pub const fn byte_rejections(self) -> u64 {
        self.byte_rejections
    }
}

/// Point-in-time mailbox state and pressure observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MailboxSnapshot {
    pub(super) control: LaneSnapshot,
    pub(super) work: LaneSnapshot,
    pub(super) live_senders: usize,
    pub(super) receiver_alive: bool,
    pub(super) closed_rejections: u64,
    pub(super) wake_failures: u64,
    pub(super) wake_requested: bool,
}

impl MailboxSnapshot {
    /// Returns the selected lane snapshot.
    pub const fn lane(self, lane: Lane) -> LaneSnapshot {
        match lane {
            Lane::Control => self.control,
            Lane::Work => self.work,
        }
    }

    /// Returns configured mailbox limits.
    pub const fn limits(self) -> MailboxLimits {
        MailboxLimits::new(self.control.limits, self.work.limits)
    }

    /// Returns live sender handles.
    pub const fn live_senders(self) -> usize {
        self.live_senders
    }

    /// Returns whether the receiver still accepts admission.
    pub const fn receiver_alive(self) -> bool {
        self.receiver_alive
    }

    /// Returns cumulative closed-mailbox rejections.
    pub const fn closed_rejections(self) -> u64 {
        self.closed_rejections
    }

    /// Returns cumulative backend wake failures.
    pub const fn wake_failures(self) -> u64 {
        self.wake_failures
    }

    /// Returns whether a wake is currently outstanding.
    pub const fn wake_requested(self) -> bool {
        self.wake_requested
    }
}
