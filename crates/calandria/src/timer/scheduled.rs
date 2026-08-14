//! Heap entry ordering by absolute deadline and monotonic admission identity.

use core::cmp::Ordering;

use super::Timer;

#[derive(Debug)]
pub(super) struct Scheduled<T> {
    pub(super) timer: Timer<T>,
}

impl<T> PartialEq for Scheduled<T> {
    fn eq(&self, other: &Self) -> bool {
        self.timer.token() == other.timer.token()
    }
}

impl<T> Eq for Scheduled<T> {}

impl<T> PartialOrd for Scheduled<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Scheduled<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .timer
            .deadline()
            .cmp(&self.timer.deadline())
            .then_with(|| other.timer.token().id().cmp(&self.timer.token().id()))
    }
}
