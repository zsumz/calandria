//! Shared timer contracts exercised with virtual time.

use std::num::NonZeroUsize;

use calandria::{Deadline, Retained, RetainedBytes, Span, TimerLimits, TimerOwnerId, TimerQueue};
use calandria_sim::VirtualClock;

#[derive(Debug, Eq, PartialEq)]
struct Event(&'static str);

impl Retained for Event {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::ZERO
    }
}

#[test]
fn production_timer_ordering_runs_against_virtual_time() {
    let mut clock = VirtualClock::new();
    let mut timers = TimerQueue::new(
        TimerOwnerId::new(1),
        TimerLimits::new(NonZeroUsize::MIN, RetainedBytes::new(1)),
    );
    let at = clock
        .now()
        .checked_add(Span::from_nanos(25))
        .unwrap_or_else(|| panic!("test deadline must fit"));
    let at = Deadline::at(at);
    let _ = timers
        .schedule(at, Event("wake"))
        .unwrap_or_else(|error| panic!("timer must fit: {error}"));

    assert!(timers.pop_due(clock.now()).is_none());
    clock
        .advance_by(Span::from_nanos(25))
        .unwrap_or_else(|error| panic!("virtual time must advance: {error}"));

    assert_eq!(
        timers
            .pop_due(clock.now())
            .map(calandria::Timer::into_value),
        Some(Event("wake"))
    );
    assert!(timers.is_empty());
}
