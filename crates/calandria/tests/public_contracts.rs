//! Public terminal diagnostics, snapshots, identities, and owned-value contracts.

use core::num::NonZeroUsize;
use std::{error::Error, fmt};

use calandria::{
    CompletionError, Deadline, HostAction, HostConfig, HostError, Interest, Moment, Next,
    PollEvent, PollEvents, ReactorFailure, Readiness, ResourceGeneration, ResourceOwnerId,
    ResourceSlotId, ResourceToken, Retained, RetainedBytes, ShutdownSubscribeError, Span, TimerId,
    TimerLimits, TimerOwnerId, TimerQueue, WorkCount,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlannedFailure(&'static str);

impl fmt::Display for PlannedFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for PlannedFailure {}

#[derive(Debug, Eq, PartialEq)]
struct Owned {
    id: u8,
    bytes: RetainedBytes,
}

impl Retained for Owned {
    fn retained_bytes(&self) -> RetainedBytes {
        self.bytes
    }
}

#[test]
fn completion_and_shutdown_errors_distinguish_terminal_causes() {
    for (error, expected) in [
        (
            CompletionError::Closed,
            "the completion producer closed without a value",
        ),
        (
            CompletionError::Consumed,
            "the completion value was already consumed",
        ),
    ] {
        assert_eq!(error.to_string(), expected);
        assert!(Error::source(&error).is_none());
    }

    let full: ShutdownSubscribeError<PlannedFailure> = ShutdownSubscribeError::Full;
    assert_eq!(full.to_string(), "shutdown subscriber capacity is full");
    assert!(Error::source(&full).is_none());
    let closed: ShutdownSubscribeError<PlannedFailure> = ShutdownSubscribeError::Closed;
    assert_eq!(closed.to_string(), "the shutdown barrier is closed");
    assert!(Error::source(&closed).is_none());
    let request = ShutdownSubscribeError::Request(PlannedFailure("request failed"));
    assert_eq!(request.to_string(), "the first shutdown request failed");
    assert_eq!(
        Error::source(&request).map(ToString::to_string),
        Some(String::from("request failed"))
    );
}

#[test]
fn reactor_failures_retain_the_host_or_waiting_source_layer() {
    type Failure = ReactorFailure<PlannedFailure, PlannedFailure, PlannedFailure>;
    let host = Failure::Host(HostError::Duty(PlannedFailure("duty failed")));
    assert_eq!(
        host.to_string(),
        "reactor host failed: host duty failed: duty failed"
    );
    assert_eq!(
        Error::source(&host).map(ToString::to_string),
        Some(String::from("host duty failed: duty failed"))
    );

    let wait = Failure::Wait(PlannedFailure("wait failed"));
    assert_eq!(wait.to_string(), "reactor wait failed: wait failed");
    assert_eq!(
        Error::source(&wait).map(ToString::to_string),
        Some(String::from("wait failed"))
    );
}

#[test]
fn timer_limits_identities_snapshots_and_values_round_trip() {
    let defaults = TimerLimits::default();
    assert_eq!(defaults.timers(), nonzero(1_024));
    assert_eq!(
        defaults.retained_bytes(),
        RetainedBytes::new(16 * 1_024 * 1_024)
    );
    let owner = TimerOwnerId::new(7);
    assert_eq!(owner.get(), 7);
    assert_eq!(TimerId::ZERO.get(), 0);
    assert_eq!(TimerId::new(9).get(), 9);

    let limits = TimerLimits::new(nonzero(2), RetainedBytes::new(8));
    let mut queue = TimerQueue::new(owner, limits);
    assert_eq!(queue.owner(), owner);
    assert_eq!(queue.limits(), limits);
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
    let deadline = Deadline::at(Moment::from_nanos(5));
    let token = queue
        .schedule(deadline, owned(1, 3))
        .unwrap_or_else(|error| panic!("timer must fit: {error}"));
    assert_eq!(token.owner(), owner);
    assert_eq!(token.id(), TimerId::ZERO);
    assert_eq!(token.deadline(), deadline);
    assert!(!queue.is_empty());
    assert_eq!(queue.len(), 1);

    let snapshot = queue.snapshot();
    assert_eq!(snapshot.owner(), owner);
    assert_eq!(snapshot.limits(), limits);
    assert_eq!(snapshot.pending_timers(), 1);
    assert_eq!(snapshot.retained_bytes(), RetainedBytes::new(3));
    assert_eq!(snapshot.next_deadline(), Some(deadline));
    assert_eq!(snapshot.next_id(), Some(TimerId::new(1)));
    assert!(!snapshot.identities_exhausted());

    let timer = queue
        .cancel(token)
        .unwrap_or_else(|| panic!("timer must remain pending"));
    assert_eq!(timer.value(), &owned(1, 3));
    assert_eq!(timer.retained_bytes(), RetainedBytes::new(3));
    assert_eq!(timer.into_parts(), (token, owned(1, 3)));
}

#[test]
fn poll_events_expose_resource_facts_capacity_and_owned_drain_order() {
    let token = ResourceToken::new(
        ResourceOwnerId::new(1),
        ResourceSlotId::new(2),
        ResourceGeneration::new(3),
    );
    let resource = PollEvent::Resource {
        token,
        readiness: Readiness::READABLE | Readiness::WRITABLE,
    };
    assert_eq!(PollEvent::Wake.resource(), None);
    assert_eq!(PollEvent::Wake.readiness(), None);
    assert_eq!(resource.resource(), Some(token));
    assert_eq!(
        resource.readiness(),
        Some(Readiness::READABLE | Readiness::WRITABLE)
    );

    let mut events = PollEvents::new(nonzero(2));
    assert_eq!(events.capacity(), nonzero(2));
    assert_eq!(events.len(), 0);
    assert!(events.is_empty());
    events
        .try_push(PollEvent::Wake)
        .unwrap_or_else(|error| panic!("wake must fit: {error}"));
    events
        .try_push(resource)
        .unwrap_or_else(|error| panic!("resource must fit: {error}"));
    assert_eq!(events.as_slice(), [PollEvent::Wake, resource]);
    assert_eq!(events.get(1), Some(&resource));
    assert_eq!(events.iter().len(), 2);
    assert_eq!((&events).into_iter().count(), 2);

    let Err(error) = events.try_push(PollEvent::Wake) else {
        panic!("third event must exceed capacity");
    };
    assert_eq!(error.event(), PollEvent::Wake);
    assert_eq!(error.capacity(), nonzero(2));
    assert_eq!(
        error.to_string(),
        "readiness batch capacity of 2 was reached"
    );
    assert!(Error::source(&error).is_none());

    let mut drain = events.drain();
    assert!(format!("{drain:?}").contains("remaining: 2"));
    assert_eq!(drain.len(), 2);
    assert_eq!(drain.next(), Some(PollEvent::Wake));
    assert_eq!(drain.next(), Some(resource));
    assert_eq!(drain.next(), None);
    drop(drain);
    assert!(events.is_empty());
    events.clear();
}

#[test]
fn host_action_wait_accessors_keep_zero_waits_immediate() {
    let now = Moment::from_nanos(5);
    let config = HostConfig::new(Span::from_nanos(9));
    assert_eq!(HostAction::Continue.wait_span(), None);
    assert_eq!(HostAction::Stop.wait_span(), None);
    assert_eq!(
        HostAction::Wait(Span::from_nanos(3)).wait_span(),
        Some(Span::from_nanos(3))
    );
    assert_eq!(
        HostAction::for_next(calandria::Next::Wake, now, config),
        HostAction::Wait(Span::from_nanos(9))
    );
    assert!(Interest::READABLE.is_readable());
}

#[test]
fn turn_primitives_expose_counts_and_all_bounded_wait_classes() {
    assert_eq!(WorkCount::new(7).get(), 7);
    let now = Moment::from_nanos(5);
    let ceiling = Span::from_nanos(9);
    assert_eq!(Next::Now.bounded_wait(now, ceiling), Span::ZERO);
    assert_eq!(Next::Wake.bounded_wait(now, ceiling), ceiling);
    assert_eq!(Next::Stop.bounded_wait(now, ceiling), ceiling);
}

const fn owned(id: u8, bytes: u64) -> Owned {
    Owned {
        id,
        bytes: RetainedBytes::new(bytes),
    }
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
