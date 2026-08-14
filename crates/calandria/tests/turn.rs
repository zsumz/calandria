//! Scheduling-interest merge and work-accounting tests.

use calandria::{Deadline, Moment, Next, Span, Turn, WorkCount};

#[test]
fn immediate_work_wins_and_counts_saturate() {
    let left = Turn::new(WorkCount::new(u64::MAX), Next::Wake);
    let right = Turn::runnable(WorkCount::new(4));

    let merged = left.merge(right);

    assert_eq!(merged.work(), WorkCount::new(u64::MAX));
    assert_eq!(merged.next(), Next::Now);
}

#[test]
fn earliest_deadline_wins_over_indefinite_wait() {
    let later = Next::WakeOr(Deadline::at(Moment::from_nanos(20)));
    let earlier = Next::WakeOr(Deadline::at(Moment::from_nanos(10)));

    assert_eq!(later.merge(Next::Wake).merge(earlier), earlier);
    assert_eq!(
        earlier.bounded_wait(Moment::from_nanos(3), Span::from_nanos(100)),
        Span::from_nanos(7)
    );
}

#[test]
fn stop_is_the_aggregation_identity() {
    assert_eq!(Next::Stop.merge(Next::Wake), Next::Wake);
    assert_eq!(Next::Stop.merge(Next::Stop), Next::Stop);
}
