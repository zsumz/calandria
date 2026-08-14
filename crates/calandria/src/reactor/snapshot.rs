//! Saturating observations for one reactor's waiting loop.

use crate::WaitOutcome;

/// Saturating observations for one reactor's waiting loop.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReactorSnapshot {
    waits: u64,
    notifications: u64,
    idle_returns: u64,
}

impl ReactorSnapshot {
    /// Returns successful wait calls.
    pub const fn waits(self) -> u64 {
        self.waits
    }

    /// Returns waits that observed an external notification.
    pub const fn notifications(self) -> u64 {
        self.notifications
    }

    /// Returns waits that returned without an external notification.
    pub const fn idle_returns(self) -> u64 {
        self.idle_returns
    }

    pub(super) fn record(&mut self, outcome: WaitOutcome) {
        self.waits = self.waits.saturating_add(1);
        match outcome {
            WaitOutcome::Notified => {
                self.notifications = self.notifications.saturating_add(1);
            }
            WaitOutcome::Idle => {
                self.idle_returns = self.idle_returns.saturating_add(1);
            }
        }
    }
}
