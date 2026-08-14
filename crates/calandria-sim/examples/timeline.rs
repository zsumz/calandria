//! Delivers finite scripted events in deterministic virtual-time order.

use core::num::NonZeroUsize;

use calandria::{Retained, RetainedBytes, Span};
use calandria_sim::{Timeline, TimelineId, TimelineLimits};

#[derive(Debug)]
struct Event(&'static str);

impl Retained for Event {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::try_from(self.0.len())
            .unwrap_or_else(|_| panic!("example string length must fit in u64"))
    }
}

fn main() {
    let limits = TimelineLimits::new(nonzero_usize(16), RetainedBytes::new(4_096));
    let mut timeline = Timeline::new(TimelineId::new(1), limits);

    timeline
        .schedule_after(Span::from_nanos(20), Event("later"))
        .unwrap_or_else(|error| panic!("schedule failed: {error}"));
    timeline
        .schedule_after(Span::from_nanos(10), Event("first"))
        .unwrap_or_else(|error| panic!("schedule failed: {error}"));
    timeline
        .schedule_after(Span::from_nanos(10), Event("same-time fifo"))
        .unwrap_or_else(|error| panic!("schedule failed: {error}"));

    while let Some(delivery) = timeline.pop_next() {
        println!("{}ns: {}", delivery.at().as_nanos(), delivery.event().0);
    }
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("example limit must be nonzero"))
}
