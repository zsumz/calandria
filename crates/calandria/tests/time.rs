//! Fixed-width monotonic time, duration conversion, and deadline arithmetic tests.

use core::time::Duration;

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
    assert!(deadline.is_elapsed_at(Moment::from_nanos(10)));
    assert!(!deadline.is_elapsed_at(Moment::from_nanos(9)));
    assert_eq!(deadline.moment(), Moment::from_nanos(10));
    assert_eq!(
        Deadline::from(Moment::from_nanos(12)).moment(),
        Moment::from_nanos(12)
    );
}

#[test]
fn span_construction_conversion_and_overflow_are_explicit() {
    let millis =
        Span::checked_from_millis(2).unwrap_or_else(|| panic!("small millisecond span must fit"));
    assert_eq!(millis.as_nanos(), 2_000_000);
    assert_eq!(millis.as_duration(), Duration::from_millis(2));
    assert_eq!(Duration::from(millis), Duration::from_millis(2));
    assert_eq!(
        Span::from_nanos(4).checked_add(Span::from_nanos(5)),
        Some(Span::from_nanos(9))
    );
    assert_eq!(
        Span::from_nanos(4).min(Span::from_nanos(5)),
        Span::from_nanos(4)
    );
    assert_eq!(
        Span::from_nanos(5).min(Span::from_nanos(4)),
        Span::from_nanos(4)
    );
    assert_eq!(
        Span::from_nanos(u64::MAX).checked_add(Span::from_nanos(1)),
        None
    );
    assert_eq!(Span::checked_from_millis(u64::MAX), None);

    let duration = Duration::new(u64::MAX, 999_999_999);
    let Err(error) = Span::try_from(duration) else {
        panic!("duration above u64 nanoseconds must reject");
    };
    assert_eq!(error.duration(), duration);
    assert_eq!(
        error.to_string(),
        format!("duration {duration:?} exceeds the u64 nanosecond time domain")
    );
}
