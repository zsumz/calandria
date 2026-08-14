//! Combines bounded timers, readiness events, and generational resources.

use std::{
    error::Error,
    num::NonZeroUsize,
};

use calandria::{
    Deadline, EventBatch, EventBatchLimits, Moment, ResourceOwnerId, ResourceTable, ResourceToken,
    Retained, RetainedBytes, TimerLimits, TimerOwnerId, TimerQueue,
};

#[derive(Debug, Eq, PartialEq)]
enum Event {
    Ready(ResourceToken),
    Deadline(ResourceToken),
}

impl Retained for Event {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::ZERO
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut resources = ResourceTable::new(ResourceOwnerId::new(0), nonzero_usize(8));
    let resource = resources.admit(7_u64, "connection")?;

    let mut timers = TimerQueue::new(TimerOwnerId::new(1), TimerLimits::new(
        nonzero_usize(32),
        RetainedBytes::new(4 * 1_024),
    ));
    let _ = timers.schedule(
        Deadline::at(Moment::from_nanos(10)),
        Event::Deadline(resource),
    )?;

    let mut ready = EventBatch::new(EventBatchLimits::new(
        nonzero_usize(16),
        RetainedBytes::new(4 * 1_024),
    ));
    ready.try_push(Event::Ready(resource))?;

    let mut events = ready.drain().collect::<Vec<_>>();
    let mut due = Vec::new();
    let _ = timers.drain_due_into(Moment::from_nanos(10), &mut due, nonzero_usize(8));
    events.extend(due.into_iter().map(calandria::Timer::into_value));

    assert_eq!(events, [Event::Ready(resource), Event::Deadline(resource)]);
    assert_eq!(resources.remove(resource), Ok((7, "connection")));
    Ok(())
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("limit must be nonzero"))
}
