//! Fixed-width monotonic time and deadline arithmetic tests.

use calandria::{Deadline, Moment, Span};

#[test]
fn moment_arithmetic_is_checked() {
    let start = Moment::from_nanos(7);
    let end = start.checked_add(Span::from_nanos(5));

    assert_eq!(end, Some(Moment::from_nanos(12)));
    assert_eq!(
        end.and_then(|value| value.duration_since(start)),
        Some(Span::from_nanos(5))
    );
    assert_eq!(start.duration_since(Moment::from_nanos(8)), None);
}

#[test]
fn deadline_remaining_is_absolute_and_clamped() {
    let deadline = Deadline::at(Moment::from_nanos(10));

    assert_eq!(
        deadline.remaining_at(Moment::from_nanos(4)),
        Span::from_nanos(6)
    );
    assert_eq!(deadline.remaining_at(Moment::from_nanos(10)), Span::ZERO);
    assert_eq!(deadline.remaining_at(Moment::from_nanos(11)), Span::ZERO);
}
