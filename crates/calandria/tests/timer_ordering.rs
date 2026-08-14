//! Stable timer ordering, cancellation, and bounded-drain tests.

use std::num::NonZeroUsize;

use calandria::{
    Deadline, Moment, Retained, RetainedBytes, TimerId, TimerLimits, TimerOwnerId, TimerQueue,
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
fn timers_fire_by_deadline_then_admission_identity() {
    let mut timers = queue(4, 64);
    let first = schedule(&mut timers, 20, Item::new("late", 3));
    let second = schedule(&mut timers, 10, Item::new("first-due", 4));
    let third = schedule(&mut timers, 10, Item::new("second-due", 5));
    let mut due = Vec::new();

    let drain = timers.drain_due_into(moment(20), &mut due, nonzero_usize(4));

    assert_eq!(first.id(), TimerId::new(0));
    assert_eq!(second.id(), TimerId::new(1));
    assert_eq!(third.id(), TimerId::new(2));
    assert_eq!(names(&due), vec!["first-due", "second-due", "late"]);
    assert_eq!(drain.fired(), 3);
    assert!(!drain.more_due());
    let snapshot = timers.snapshot();
    assert_eq!(snapshot.pending_timers(), 0);
    assert_eq!(snapshot.retained_bytes(), RetainedBytes::ZERO);
    assert_eq!(snapshot.next_id(), Some(TimerId::new(3)));
}

#[test]
fn bounded_drain_retains_due_and_future_timers() {
    let mut timers = queue(3, 64);
    schedule(&mut timers, 10, Item::new("one", 1));
    schedule(&mut timers, 10, Item::new("two", 1));
    schedule(&mut timers, 30, Item::new("future", 1));
    let mut due = Vec::new();

    let first = timers.drain_due_into(moment(20), &mut due, nonzero_usize(1));

    assert_eq!(names(&due), vec!["one"]);
    assert_eq!(first.fired(), 1);
    assert!(first.more_due());
    assert_eq!(timers.next_deadline(), Some(deadline(10)));

    let second = timers.drain_due_into(moment(20), &mut due, nonzero_usize(2));

    assert_eq!(names(&due), vec!["one", "two"]);
    assert_eq!(second.fired(), 1);
    assert!(!second.more_due());
    assert_eq!(timers.next_deadline(), Some(deadline(30)));
}

#[test]
fn cancellation_returns_exact_ownership_and_restores_accounting() {
    let mut timers = queue(2, 64);
    let canceled = schedule(&mut timers, 10, Item::new("cancel", 7));
    schedule(&mut timers, 20, Item::new("keep", 11));

    let removed = timers
        .cancel(canceled)
        .unwrap_or_else(|| panic!("scheduled token must cancel"));

    assert_eq!(removed.token(), canceled);
    assert_eq!(removed.deadline(), deadline(10));
    assert_eq!(removed.measured_retained_bytes(), RetainedBytes::new(7));
    assert_eq!(removed.into_value(), Item::new("cancel", 7));
    assert!(timers.cancel(canceled).is_none());
    assert_eq!(timers.next_deadline(), Some(deadline(20)));
    assert_eq!(timers.snapshot().retained_bytes(), RetainedBytes::new(11));
}

#[test]
fn cancellation_rebuild_preserves_equal_deadline_order() {
    let mut timers = queue(3, 64);
    let first = schedule(&mut timers, 10, Item::new("first", 1));
    let middle = schedule(&mut timers, 10, Item::new("middle", 1));
    let last = schedule(&mut timers, 10, Item::new("last", 1));

    let removed = timers
        .cancel(middle)
        .unwrap_or_else(|| panic!("middle timer must cancel"));
    assert_eq!(removed.into_value(), Item::new("middle", 1));

    let mut due = Vec::new();
    let drain = timers.drain_due_into(moment(10), &mut due, nonzero_usize(3));

    assert_eq!(names(&due), vec!["first", "last"]);
    assert_eq!(due[0].token(), first);
    assert_eq!(due[1].token(), last);
    assert_eq!(drain.fired(), 2);
    assert!(!drain.more_due());
}

#[test]
fn foreign_queue_tokens_cannot_cancel_matching_local_timers() {
    let mut first = queue_for(TimerOwnerId::new(1), 1, 0);
    let mut second = queue_for(TimerOwnerId::new(2), 1, 0);
    let foreign = schedule(&mut first, 10, Item::new("first", 0));
    let local = schedule(&mut second, 10, Item::new("second", 0));

    assert!(second.cancel(foreign).is_none());
    assert_eq!(
        second.cancel(local).map(calandria::Timer::into_value),
        Some(Item::new("second", 0))
    );
}

#[test]
fn canceled_tokens_are_never_reused() {
    let mut timers = queue(1, 64);
    let stale = schedule(&mut timers, 10, Item::new("old", 1));
    let _ = timers
        .cancel(stale)
        .unwrap_or_else(|| panic!("old timer must cancel"));
    let current = schedule(&mut timers, 10, Item::new("new", 1));

    assert_ne!(stale, current);
    assert!(timers.cancel(stale).is_none());
    assert_eq!(
        timers.cancel(current).map(|timer| timer.into_value().name),
        Some("new")
    );
}

fn queue(count: usize, retained: u64) -> TimerQueue<Item> {
    queue_for(TimerOwnerId::new(1), count, retained)
}

fn queue_for(owner: TimerOwnerId, count: usize, retained: u64) -> TimerQueue<Item> {
    TimerQueue::new(
        owner,
        TimerLimits::new(nonzero_usize(count), RetainedBytes::new(retained)),
    )
}

fn schedule(timers: &mut TimerQueue<Item>, at: u64, item: Item) -> calandria::TimerToken {
    timers
        .schedule(deadline(at), item)
        .unwrap_or_else(|error| panic!("timer must fit: {error}"))
}

fn names(timers: &[calandria::Timer<Item>]) -> Vec<&'static str> {
    timers.iter().map(|timer| timer.value().name).collect()
}

fn deadline(raw: u64) -> Deadline {
    Deadline::at(moment(raw))
}

fn moment(raw: u64) -> Moment {
    Moment::from_nanos(raw)
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test count must be nonzero"))
}
