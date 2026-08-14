//! Bounded readiness polling contract.

use crate::Span;

use super::PollEvents;

/// Result of one bounded readiness observation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PollReport {
    observed: usize,
    delivered: usize,
    wakes: usize,
    stale: usize,
    saturated: bool,
}

impl PollReport {
    /// Creates a report from exact delivered and rejected observations.
    ///
    /// `wakes` must be no greater than `delivered`. Raw observations are
    /// derived as `delivered + stale`, keeping report accounting consistent by
    /// construction.
    ///
    /// # Panics
    ///
    /// Panics when `wakes` exceeds `delivered` or the raw observation count
    /// cannot be represented by `usize`. Both conditions indicate a broken
    /// poller implementation rather than runtime pressure.
    pub const fn new(
        delivered: usize,
        wakes: usize,
        stale: usize,
        saturated: bool,
    ) -> Self {
        assert!(wakes <= delivered, "wake count exceeds delivered events");
        let observed = match delivered.checked_add(stale) {
            Some(observed) => observed,
            None => panic!("poll report observation count overflowed"),
        };
        Self {
            observed,
            delivered,
            wakes,
            stale,
            saturated,
        }
    }

    /// Returns raw backend events observed.
    pub const fn observed(self) -> usize {
        self.observed
    }

    /// Returns events delivered to the bounded destination.
    pub const fn delivered(self) -> usize {
        self.delivered
    }

    /// Returns delivered administrative wake events.
    pub const fn wakes(self) -> usize {
        self.wakes
    }

    /// Returns delivered resource-readiness events.
    pub const fn resources(self) -> usize {
        self.delivered - self.wakes
    }

    /// Returns stale backend events rejected before delivery.
    pub const fn stale(self) -> usize {
        self.stale
    }

    /// Returns whether the backend event batch filled its observation budget.
    pub const fn saturated(self) -> bool {
        self.saturated
    }
}

/// One owner-local readiness backend.
pub trait Poller {
    /// Failure returned by the backend.
    type Error;

    /// Replaces `destination` with one bounded readiness observation.
    ///
    /// The implementation clears `destination` before observing its backend.
    /// On success, the destination contains exactly the delivered events. On
    /// failure, it remains empty. The call may return early or spuriously. It
    /// must use `maximum` as the backend blocking bound rather than intentionally
    /// extending it; platform clock granularity and scheduling may delay the
    /// return. It may not retain more observations than the destination can
    /// hold or invoke domain policy. A saturated report tells
    /// the owner to arrange an immediate follow-up opportunity.
    fn poll(
        &mut self,
        maximum: Span,
        destination: &mut PollEvents,
    ) -> Result<PollReport, Self::Error>;
}
