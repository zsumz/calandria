//! Bounded timeline ownership, ordering, and accounting tests.

use core::{error::Error, num::NonZeroUsize};

use calandria::{Moment, Retained, RetainedBytes, Span};
use calandria_sim::{Planned, ScheduleFailure, Timeline, TimelineId, TimelineLimits};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Event {
    id: u8,
    bytes: u64,
}

impl Retained for Event {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::new(self.bytes)
    }
}

#[test]
fn equal_time_events_are_delivered_in_insertion_order() {
    let mut timeline = Timeline::new(TimelineId::new(1), limits(8, 64));
    assert!(
        timeline
            .schedule_after(Span::from_nanos(3), event(1, 1))
            .is_ok()
    );
    assert!(
        timeline
            .schedule_after(Span::from_nanos(3), event(2, 1))
            .is_ok()
    );
    assert!(
        timeline
            .schedule_after(Span::from_nanos(3), event(3, 1))
            .is_ok()
    );

    let mut delivered = Vec::new();
    while let Some(next) = timeline.pop_next() {
        delivered.push(next.into_event());
    }

    assert_eq!(delivered, [event(1, 1), event(2, 1), event(3, 1)]);
    assert_eq!(timeline.now(), Moment::from_nanos(3));
}

#[test]
fn count_rejection_preserves_event_ownership() {
    let mut timeline = Timeline::new(TimelineId::new(1), limits(1, 64));
    assert!(timeline.schedule_after(Span::ZERO, event(1, 1)).is_ok());

    let Err(error) = timeline.schedule_after(Span::ZERO, event(2, 1)) else {
        panic!("event count limit must reject");
    };

    assert_eq!(error.to_string(), "pending event capacity of 1 was reached");
    assert!(Error::source(&error).is_some());
    assert!(matches!(
        error.failure(),
        ScheduleFailure::EventCapacity { .. }
    ));
    assert_eq!(error.into_event(), event(2, 1));
    assert_eq!(timeline.snapshot().pending_events(), 1);
}

#[test]
fn byte_rejection_preserves_event_ownership() {
    let mut timeline = Timeline::new(TimelineId::new(1), limits(2, 4));
    assert!(timeline.schedule_after(Span::ZERO, event(1, 4)).is_ok());

    let Err(error) = timeline.schedule_after(Span::ZERO, event(2, 1)) else {
        panic!("retained-byte limit must reject");
    };

    assert!(matches!(
        error.failure(),
        ScheduleFailure::RetainedByteCapacity { .. }
    ));
    assert_eq!(error.into_event(), event(2, 1));
    assert_eq!(timeline.snapshot().retained_bytes(), RetainedBytes::new(4));
}

#[test]
fn retained_byte_overflow_is_distinct_from_capacity() {
    let mut timeline = Timeline::new(TimelineId::new(1), limits(2, u64::MAX));
    assert!(
        timeline
            .schedule_after(Span::ZERO, event(1, u64::MAX))
            .is_ok()
    );

    let Err(error) = timeline.schedule_after(Span::ZERO, event(2, 1)) else {
        panic!("retained-byte addition must not wrap");
    };

    assert!(matches!(
        error.failure(),
        ScheduleFailure::RetainedByteOverflow { .. }
    ));
    assert_eq!(error.into_event(), event(2, 1));
    assert_eq!(
        timeline.snapshot().retained_bytes(),
        RetainedBytes::new(u64::MAX)
    );
}

#[test]
fn relative_time_overflow_preserves_event_ownership() {
    let mut timeline = Timeline::at(
        TimelineId::new(1),
        Moment::from_nanos(u64::MAX),
        limits(1, 1),
    );
    let Err(error) = timeline.schedule_after(Span::from_nanos(1), event(1, 1)) else {
        panic!("virtual time must not wrap");
    };

    assert!(matches!(
        error.failure(),
        ScheduleFailure::TimeOverflow { .. }
    ));
    assert_eq!(error.into_event(), event(1, 1));
    assert!(timeline.is_empty());
    assert_eq!(timeline.now(), Moment::from_nanos(u64::MAX));
}

#[test]
fn cancellation_returns_ownership_and_restores_accounting() {
    let mut timeline = Timeline::new(TimelineId::new(1), limits(2, 8));
    let token = timeline
        .schedule_after(Span::from_nanos(5), event(7, 8))
        .unwrap_or_else(|_| panic!("event should fit"));

    assert_eq!(timeline.cancel(token), Some(event(7, 8)));
    assert_eq!(timeline.snapshot().pending_events(), 0);
    assert_eq!(timeline.snapshot().retained_bytes(), RetainedBytes::ZERO);
    assert_eq!(timeline.cancel(token), None);
}

#[test]
fn foreign_timeline_tokens_cannot_cancel_matching_local_events() {
    let mut first = Timeline::new(TimelineId::new(1), limits(1, 0));
    let mut second = Timeline::new(TimelineId::new(2), limits(1, 0));
    let foreign = first
        .schedule_at(Moment::from_nanos(10), event(1, 0))
        .unwrap_or_else(|error| panic!("first event must fit: {error}"));
    let local = second
        .schedule_at(Moment::from_nanos(10), event(2, 0))
        .unwrap_or_else(|error| panic!("second event must fit: {error}"));

    assert!(second.cancel(foreign).is_none());
    assert_eq!(second.cancel(local), Some(event(2, 0)));
}

#[test]
fn zero_byte_timeline_accepts_only_fixed_size_events() {
    let mut timeline = Timeline::new(TimelineId::new(1), limits(2, 0));
    assert!(timeline.schedule_after(Span::ZERO, event(1, 0)).is_ok());

    let Err(error) = timeline.schedule_after(Span::ZERO, event(2, 1)) else {
        panic!("retained event must be rejected");
    };

    assert!(matches!(
        error.failure(),
        ScheduleFailure::RetainedByteCapacity { limit, .. }
            if limit == RetainedBytes::ZERO
    ));
}

#[test]
fn scheduling_in_the_past_is_rejected_without_mutation() {
    let mut timeline = Timeline::at(TimelineId::new(1), Moment::from_nanos(10), limits(2, 8));
    let Err(error) = timeline.schedule_at(Moment::from_nanos(9), event(1, 1)) else {
        panic!("past event must reject");
    };

    assert!(matches!(
        error.failure(),
        ScheduleFailure::ScheduledInPast { .. }
    ));
    assert_eq!(timeline.snapshot().pending_events(), 0);
    assert_eq!(timeline.now(), Moment::from_nanos(10));
}

#[test]
fn planned_outcomes_schedule_with_their_owned_delay() {
    let mut timeline = Timeline::new(TimelineId::new(1), limits(1, 8));
    let token = timeline
        .schedule_planned(Planned::new(Span::from_nanos(7), event(1, 3)))
        .unwrap_or_else(|error| panic!("planned event must fit: {error}"));

    assert_eq!(token.at(), Moment::from_nanos(7));
    assert_eq!(
        timeline.pop_next().map(calandria_sim::Delivery::into_event),
        Some(event(1, 3))
    );
}

#[test]
fn explicit_measurement_supports_foreign_events_at_origin_and_restored_time() {
    let measure = |event: &String| {
        RetainedBytes::try_from(event.len())
            .unwrap_or_else(|_| panic!("test event length must fit retained accounting"))
    };
    let mut origin = Timeline::with_measure(TimelineId::new(2), limits(1, 4), measure);
    origin
        .schedule_after(Span::ZERO, String::from("four"))
        .unwrap_or_else(|error| panic!("measured event must fit: {error}"));
    assert_eq!(origin.snapshot().retained_bytes(), RetainedBytes::new(4));

    let mut restored = Timeline::at_with_measure(
        TimelineId::new(3),
        Moment::from_nanos(9),
        limits(1, 5),
        measure,
    );
    restored
        .schedule_after(Span::from_nanos(1), String::from("five!"))
        .unwrap_or_else(|error| panic!("restored measured event must fit: {error}"));
    assert_eq!(restored.now(), Moment::from_nanos(9));
    assert_eq!(restored.snapshot().retained_bytes(), RetainedBytes::new(5));
}

fn limits(events: usize, bytes: u64) -> TimelineLimits {
    TimelineLimits::new(nonzero_usize(events), RetainedBytes::new(bytes))
}

const fn event(id: u8, bytes: u64) -> Event {
    Event { id, bytes }
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test count must be nonzero"))
}
