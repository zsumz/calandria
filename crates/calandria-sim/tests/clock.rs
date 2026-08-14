//! Virtual monotonic clock boundary tests.

use calandria::{Moment, Span};
use calandria_sim::{ClockError, VirtualClock};

#[test]
fn time_never_moves_backward() {
    let mut clock = VirtualClock::at(Moment::from_nanos(10));

    assert!(matches!(
        clock.advance_to(Moment::from_nanos(9)),
        Err(ClockError::MovesBackward { .. })
    ));
    assert_eq!(clock.now(), Moment::from_nanos(10));
}

#[test]
fn time_arithmetic_is_checked() {
    let mut clock = VirtualClock::at(Moment::from_nanos(u64::MAX));

    assert!(matches!(
        clock.advance_by(Span::from_nanos(1)),
        Err(ClockError::Overflow { .. })
    ));
    assert_eq!(clock.now(), Moment::from_nanos(u64::MAX));
}
