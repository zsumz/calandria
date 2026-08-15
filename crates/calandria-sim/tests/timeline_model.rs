//! Bounded generated-operation differential model for the event timeline.

use std::{collections::BTreeMap, num::NonZeroUsize};

use calandria::{Moment, Retained, RetainedBytes};
use calandria_sim::{EventToken, ScheduleFailure, Timeline, TimelineId, TimelineLimits};

const EVENT_LIMIT: usize = 16;
const BYTE_LIMIT: u64 = 48;
const SEEDS: u64 = 32;
const OPERATIONS: usize = 192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Event {
    id: u64,
    bytes: u64,
}

impl Retained for Event {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::new(self.bytes)
    }
}

#[derive(Debug)]
struct Reference {
    now: Moment,
    retained: u64,
    next_id: u64,
    events: BTreeMap<(Moment, u64), Event>,
    issued: Vec<EventToken>,
}

impl Reference {
    fn new() -> Self {
        Self {
            now: Moment::ORIGIN,
            retained: 0,
            next_id: 0,
            events: BTreeMap::new(),
            issued: Vec::new(),
        }
    }
}

#[test]
fn generated_timeline_operations_match_a_simple_reference_owner() {
    for seed in 0..SEEDS {
        exercise_seed(seed);
    }
}

fn exercise_seed(seed: u64) {
    let id = TimelineId::new(seed + 1);
    let limits = TimelineLimits::new(nonzero(EVENT_LIMIT), RetainedBytes::new(BYTE_LIMIT));
    let mut timeline = Timeline::new(id, limits);
    let mut reference = Reference::new();
    let mut generator = Generator::new(seed);

    for operation in 0..OPERATIONS {
        match generator.next() % 3 {
            0 => schedule(&mut timeline, &mut reference, id, operation, &mut generator),
            1 => cancel(&mut timeline, &mut reference, &mut generator),
            _ => pop(&mut timeline, &mut reference),
        }
        assert_state(&timeline, &reference, id, limits);
    }
    while !reference.events.is_empty() {
        pop(&mut timeline, &mut reference);
        assert_state(&timeline, &reference, id, limits);
    }
}

fn schedule(
    timeline: &mut Timeline<Event>,
    reference: &mut Reference,
    id: TimelineId,
    operation: usize,
    generator: &mut Generator,
) {
    let delay = generator.next() % 13;
    let at = reference
        .now
        .checked_add(calandria::Span::from_nanos(delay))
        .unwrap_or_else(|| panic!("bounded model time must fit"));
    let event = Event {
        id: u64::try_from(operation).unwrap_or_else(|_| panic!("operation must fit u64")),
        bytes: generator.next() % 8,
    };
    let result = timeline.schedule_at(at, event);

    if reference.events.len() >= EVENT_LIMIT {
        let error = result.unwrap_err_or_else(|| panic!("full timeline must reject"));
        assert!(matches!(
            error.failure(),
            ScheduleFailure::EventCapacity { .. }
        ));
        assert_eq!(error.into_event(), event);
        return;
    }
    if reference.retained + event.bytes > BYTE_LIMIT {
        let error = result.unwrap_err_or_else(|| panic!("byte-full timeline must reject"));
        assert!(matches!(
            error.failure(),
            ScheduleFailure::RetainedByteCapacity { .. }
        ));
        assert_eq!(error.into_event(), event);
        return;
    }

    let token = result.unwrap_or_else(|error| panic!("modeled event must fit: {error}"));
    assert_eq!(token.timeline(), id);
    assert_eq!(token.id().get(), reference.next_id);
    assert_eq!(token.at(), at);
    reference.next_id += 1;
    reference.retained += event.bytes;
    assert!(
        reference
            .events
            .insert((at, token.id().get()), event)
            .is_none()
    );
    reference.issued.push(token);
}

fn cancel(timeline: &mut Timeline<Event>, reference: &mut Reference, generator: &mut Generator) {
    if reference.issued.is_empty() {
        assert!(timeline.is_empty());
        return;
    }
    let index = usize::try_from(generator.next()).unwrap_or(usize::MAX) % reference.issued.len();
    let token = reference.issued[index];
    let expected = reference.events.remove(&(token.at(), token.id().get()));
    let actual = timeline.cancel(token);
    assert_eq!(actual, expected);
    if let Some(event) = expected {
        reference.retained -= event.bytes;
    }
}

fn pop(timeline: &mut Timeline<Event>, reference: &mut Reference) {
    let expected = reference.events.pop_first();
    let actual = timeline.pop_next();
    match (actual, expected) {
        (None, None) => {}
        (Some(delivery), Some(((at, id), event))) => {
            assert_eq!(delivery.at(), at);
            assert_eq!(delivery.token().id().get(), id);
            assert_eq!(delivery.event(), &event);
            let (token, actual) = delivery.into_parts();
            assert_eq!(token.at(), at);
            assert_eq!(actual, event);
            reference.now = at;
            reference.retained -= event.bytes;
        }
        (actual, expected) => panic!("timeline/reference pop mismatch: {actual:?} {expected:?}"),
    }
}

fn assert_state(
    timeline: &Timeline<Event>,
    reference: &Reference,
    id: TimelineId,
    limits: TimelineLimits,
) {
    let snapshot = timeline.snapshot();
    assert_eq!(timeline.id(), id);
    assert_eq!(timeline.now(), reference.now);
    assert_eq!(timeline.len(), reference.events.len());
    assert_eq!(timeline.is_empty(), reference.events.is_empty());
    assert_eq!(
        timeline.next_at(),
        reference.events.first_key_value().map(|(key, _)| key.0)
    );
    assert_eq!(snapshot.id(), id);
    assert_eq!(snapshot.limits(), limits);
    assert_eq!(snapshot.now(), reference.now);
    assert_eq!(snapshot.pending_events(), reference.events.len());
    assert_eq!(
        snapshot.retained_bytes(),
        RetainedBytes::new(reference.retained)
    );
    assert_eq!(snapshot.next_at(), timeline.next_at());
}

#[derive(Debug)]
struct Generator(u64);

impl Generator {
    const fn new(seed: u64) -> Self {
        Self(seed ^ 0xa076_1d64_78bd_642f)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
}

trait ResultExt<T, E> {
    fn unwrap_err_or_else(self, on_ok: impl FnOnce() -> E) -> E;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn unwrap_err_or_else(self, on_ok: impl FnOnce() -> E) -> E {
        match self {
            Ok(_) => on_ok(),
            Err(error) => error,
        }
    }
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
