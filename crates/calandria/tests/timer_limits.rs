use std::num::NonZeroUsize;

use calandria::{
    Deadline, Moment, Retained, RetainedBytes, TimerId, TimerLimits, TimerOwnerId, TimerQueue,
    TimerScheduleFailure,
};

#[derive(Debug, Eq, PartialEq)]
struct Item {
    name: &'static str,
    retained: RetainedBytes,
}

impl Item {
    const fn new(name: &'static str, retained: u64) -> Self {
        Self {
            name,
            retained: RetainedBytes::new(retained),
        }
    }
}

impl Retained for Item {
    fn retained_bytes(&self) -> RetainedBytes {
        self.retained
    }
}

#[test]
fn count_rejection_preserves_deadline_value_and_existing_state() {
    let mut timers = queue(1, 64);
    let _ = timers
        .schedule(deadline(20), Item::new("kept", 4))
        .unwrap_or_else(|error| panic!("first timer must fit: {error}"));

    let Err(error) = timers.schedule(deadline(5), Item::new("returned", 3)) else {
        panic!("second timer must exceed count");
    };

    assert_eq!(
        error.failure(),
        TimerScheduleFailure::TimerCapacity {
            limit: nonzero_usize(1),
        }
    );
    assert_eq!(error.deadline(), deadline(5));
    assert_eq!(error.value(), &Item::new("returned", 3));
    assert_eq!(error.into_value(), Item::new("returned", 3));
    assert_eq!(timers.next_deadline(), Some(deadline(20)));
    let snapshot = timers.snapshot();
    assert_eq!(snapshot.retained_bytes(), RetainedBytes::new(4));
    assert_eq!(snapshot.next_id(), Some(TimerId::new(1)));
}

#[test]
fn byte_capacity_rejection_is_exact_and_non_mutating() {
    let mut timers = queue(2, 5);
    let _ = timers
        .schedule(deadline(10), Item::new("kept", 4))
        .unwrap_or_else(|error| panic!("first timer must fit: {error}"));

    let Err(error) = timers.schedule(deadline(20), Item::new("returned", 2)) else {
        panic!("second timer must exceed bytes");
    };

    assert_eq!(
        error.failure(),
        TimerScheduleFailure::RetainedByteCapacity {
            limit: RetainedBytes::new(5),
            current: RetainedBytes::new(4),
            timer: RetainedBytes::new(2),
        }
    );
    assert_eq!(error.into_value(), Item::new("returned", 2));
    let snapshot = timers.snapshot();
    assert_eq!(snapshot.pending_timers(), 1);
    assert_eq!(snapshot.retained_bytes(), RetainedBytes::new(4));
    assert_eq!(snapshot.next_id(), Some(TimerId::new(1)));
}

#[test]
fn zero_byte_queue_accepts_only_fixed_size_values() {
    let mut timers = queue(2, 0);
    assert!(timers.schedule(deadline(10), Item::new("fixed", 0)).is_ok());

    let error = match timers.schedule(deadline(20), Item::new("retained", 1)) {
        Ok(_) => panic!("retained timer must be rejected"),
        Err(error) => error,
    };

    assert!(matches!(
        error.failure(),
        TimerScheduleFailure::RetainedByteCapacity { limit, .. }
            if limit == RetainedBytes::ZERO
    ));
    assert_eq!(error.into_value(), Item::new("retained", 1));
}

#[test]
fn retained_accounting_overflow_is_distinct_from_capacity() {
    let mut timers = queue(2, u64::MAX);
    let token = timers
        .schedule(deadline(10), Item::new("max", u64::MAX))
        .unwrap_or_else(|error| panic!("maximum retained value must fit: {error}"));

    let Err(error) = timers.schedule(deadline(20), Item::new("overflow", 1)) else {
        panic!("addition must overflow fixed-width accounting");
    };

    assert_eq!(
        error.failure(),
        TimerScheduleFailure::RetainedByteOverflow {
            current: RetainedBytes::new(u64::MAX),
            timer: RetainedBytes::new(1),
        }
    );
    assert_eq!(error.into_value(), Item::new("overflow", 1));
    assert_eq!(timers.snapshot().next_id(), Some(TimerId::new(1)));
    assert!(timers.cancel(token).is_some());
    assert_eq!(timers.snapshot().retained_bytes(), RetainedBytes::ZERO);
}

#[test]
fn identity_exhaustion_is_explicit_after_the_last_token() {
    let limits = TimerLimits::new(nonzero_usize(2), RetainedBytes::new(64));
    let mut timers = TimerQueue::starting_at(TimerOwnerId::new(1), limits, TimerId::new(u64::MAX));
    let last = timers
        .schedule(deadline(10), Item::new("last", 1))
        .unwrap_or_else(|error| panic!("last identity must be usable: {error}"));

    assert_eq!(last.id(), TimerId::new(u64::MAX));
    assert!(timers.snapshot().identities_exhausted());
    let _ = timers
        .cancel(last)
        .unwrap_or_else(|| panic!("last timer must cancel"));

    let Err(error) = timers.schedule(deadline(20), Item::new("returned", 1)) else {
        panic!("exhausted identity space must reject admission");
    };

    assert_eq!(error.failure(), TimerScheduleFailure::TimerIdsExhausted);
    assert_eq!(error.deadline(), deadline(20));
    assert_eq!(error.into_value(), Item::new("returned", 1));
    assert!(timers.is_empty());
}

fn queue(count: usize, retained: u64) -> TimerQueue<Item> {
    TimerQueue::new(TimerOwnerId::new(1), TimerLimits::new(
        nonzero_usize(count),
        RetainedBytes::new(retained),
    ))
}

fn deadline(raw: u64) -> Deadline {
    Deadline::at(Moment::from_nanos(raw))
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test count must be nonzero"))
}
