//! Bounded event-batch ownership, ordering, and accounting tests.

use std::num::NonZeroUsize;

use calandria::{EventBatch, EventBatchFailure, EventBatchLimits, Retained, RetainedBytes};

#[derive(Debug, Eq, PartialEq)]
struct Event {
    id: u8,
    retained: RetainedBytes,
}

impl Event {
    const fn new(id: u8, retained: u64) -> Self {
        Self {
            id,
            retained: RetainedBytes::new(retained),
        }
    }
}

impl Retained for Event {
    fn retained_bytes(&self) -> RetainedBytes {
        self.retained
    }
}

#[test]
fn insertion_pop_and_drain_preserve_owner_order() {
    let mut batch = batch(4, 64);
    push(&mut batch, Event::new(1, 3));
    push(&mut batch, Event::new(2, 5));
    push(&mut batch, Event::new(3, 7));

    assert_eq!(batch.len(), 3);
    assert_eq!(
        batch.iter().map(|event| event.id).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(batch.pop(), Some(Event::new(3, 7)));
    assert_eq!(batch.snapshot().retained_bytes(), RetainedBytes::new(8));

    let mut drain = batch.drain();
    assert!(format!("{drain:?}").contains("remaining: 2"));
    assert_eq!(drain.len(), 2);
    assert_eq!(drain.next_back(), Some(Event::new(2, 5)));
    assert_eq!(drain.next(), Some(Event::new(1, 3)));
    assert_eq!(drain.next(), None);
    drop(drain);

    assert!(batch.is_empty());
    assert_eq!(batch.snapshot().retained_bytes(), RetainedBytes::ZERO);
}

#[test]
fn count_rejection_returns_the_original_event_without_mutation() {
    let mut batch = batch(1, 64);
    push(&mut batch, Event::new(1, 4));

    let Err(error) = batch.try_push(Event::new(2, 7)) else {
        panic!("second event must exceed count");
    };

    assert_eq!(
        error.failure(),
        EventBatchFailure::EventCapacity {
            limit: nonzero_usize(1),
        }
    );
    assert_eq!(error.into_event(), Event::new(2, 7));
    assert_eq!(batch.iter().map(|event| event.id).collect::<Vec<_>>(), [1]);
    assert_eq!(batch.snapshot().retained_bytes(), RetainedBytes::new(4));
}

#[test]
fn byte_rejection_and_overflow_are_distinct() {
    let mut bounded = batch(2, 5);
    push(&mut bounded, Event::new(1, 4));

    let Err(capacity) = bounded.try_push(Event::new(2, 2)) else {
        panic!("event must exceed retained-byte capacity");
    };
    assert_eq!(
        capacity.failure(),
        EventBatchFailure::RetainedByteCapacity {
            limit: RetainedBytes::new(5),
            current: RetainedBytes::new(4),
            event: RetainedBytes::new(2),
        }
    );
    assert_eq!(capacity.into_event(), Event::new(2, 2));

    let mut overflowing = batch(2, u64::MAX);
    push(&mut overflowing, Event::new(1, u64::MAX));
    let Err(overflow) = overflowing.try_push(Event::new(2, 1)) else {
        panic!("accounting addition must overflow");
    };
    assert_eq!(
        overflow.failure(),
        EventBatchFailure::RetainedByteOverflow {
            current: RetainedBytes::new(u64::MAX),
            event: RetainedBytes::new(1),
        }
    );
    assert_eq!(overflow.into_event(), Event::new(2, 1));
}

#[test]
fn zero_byte_limit_accepts_only_fixed_size_events() {
    let mut batch = batch(2, 0);
    push(&mut batch, Event::new(1, 0));

    let error = match batch.try_push(Event::new(2, 1)) {
        Ok(()) => panic!("retained event must be rejected"),
        Err(error) => error,
    };

    assert!(matches!(
        error.failure(),
        EventBatchFailure::RetainedByteCapacity { limit, .. }
            if limit == RetainedBytes::ZERO
    ));
    assert_eq!(error.into_event(), Event::new(2, 1));
}

#[test]
fn clear_restores_empty_accounting_and_reuses_storage_contract() {
    let limits = EventBatchLimits::new(nonzero_usize(2), RetainedBytes::new(8));
    let mut batch = EventBatch::new(limits);
    push(&mut batch, Event::new(1, 8));

    batch.clear();
    push(&mut batch, Event::new(2, 8));

    let snapshot = batch.snapshot();
    assert_eq!(snapshot.limits(), limits);
    assert_eq!(snapshot.events(), 1);
    assert_eq!(snapshot.retained_bytes(), RetainedBytes::new(8));
    assert_eq!(batch.first(), Some(&Event::new(2, 8)));
}

#[test]
fn explicit_measurement_accepts_a_foreign_event_type() {
    let limits = EventBatchLimits::new(nonzero_usize(2), RetainedBytes::new(3));
    let mut batch = EventBatch::<Vec<u8>>::with_measure(limits, |event| {
        RetainedBytes::try_from(event.len())
            .unwrap_or_else(|_| panic!("test event length must fit retained accounting"))
    });

    batch
        .try_push(vec![1, 2, 3])
        .unwrap_or_else(|error| panic!("measured event must fit: {error}"));
    let Err(error) = batch.try_push(vec![4]) else {
        panic!("measured event must exceed retained-byte capacity");
    };

    assert_eq!(error.into_event(), vec![4]);
    assert_eq!(batch.snapshot().retained_bytes(), RetainedBytes::new(3));
}

fn batch(events: usize, retained: u64) -> EventBatch<Event> {
    EventBatch::new(EventBatchLimits::new(
        nonzero_usize(events),
        RetainedBytes::new(retained),
    ))
}

fn push(batch: &mut EventBatch<Event>, event: Event) {
    batch
        .try_push(event)
        .unwrap_or_else(|error| panic!("event must fit: {error}"));
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test count must be nonzero"))
}
