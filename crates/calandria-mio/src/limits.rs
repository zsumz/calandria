//! Fixed operating-system event and registration limits.

use core::num::NonZeroUsize;

/// Hard limits for one Mio poller owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MioPollerLimits {
    events: NonZeroUsize,
    registrations: NonZeroUsize,
}

impl MioPollerLimits {
    /// Creates poller limits.
    pub const fn new(events: NonZeroUsize, registrations: NonZeroUsize) -> Self {
        Self {
            events,
            registrations,
        }
    }

    /// Returns the maximum events observed by one operating-system poll.
    pub const fn events(self) -> NonZeroUsize {
        self.events
    }

    /// Returns the maximum active source registrations.
    pub const fn registrations(self) -> NonZeroUsize {
        self.registrations
    }
}
