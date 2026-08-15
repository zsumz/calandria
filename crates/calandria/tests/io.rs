//! Backend-neutral readiness and bounded poll contract tests.

use core::num::NonZeroUsize;

use calandria::{
    Interest, PollEvent, PollEvents, PollReport, Readiness, ResourceGeneration, ResourceOwnerId,
    ResourceSlotId, ResourceToken,
};

#[test]
fn interest_is_nonempty_extensible_and_explicit() {
    assert!(Interest::READABLE.is_readable());
    assert!(!Interest::READABLE.is_writable());
    assert!(Interest::WRITABLE.is_writable());
    assert!(!Interest::WRITABLE.is_readable());

    let combined = Interest::READ_WRITE | Interest::PRIORITY | Interest::AIO;
    assert!(combined.is_readable());
    assert!(combined.is_writable());
    assert!(combined.is_priority());
    assert!(combined.is_aio());
    assert!(!combined.is_lio());
    assert!(combined.contains(Interest::READ_WRITE));
    assert!(combined.intersects(Interest::PRIORITY));
    assert_eq!(
        combined.remove(Interest::PRIORITY),
        Some(Interest::READ_WRITE | Interest::AIO)
    );
    assert_eq!(Interest::READABLE.remove(Interest::READABLE), None);
    let mut assigned = Interest::READABLE;
    assigned |= Interest::WRITABLE;
    assert_eq!(assigned, Interest::READ_WRITE);
    assert_eq!(
        format!("{combined:?}"),
        "READABLE | WRITABLE | PRIORITY | AIO"
    );
}

#[test]
fn readiness_retains_independent_backend_hints() {
    let readiness = Readiness::READABLE
        | Readiness::WRITE_CLOSED
        | Readiness::ERROR
        | Readiness::PRIORITY
        | Readiness::AIO
        | Readiness::LIO;

    assert!(readiness.is_readable());
    assert!(!readiness.is_writable());
    assert!(!readiness.is_read_closed());
    assert!(readiness.is_write_closed());
    assert!(readiness.is_closed());
    assert!(readiness.is_error());
    assert!(readiness.is_priority());
    assert!(readiness.is_aio());
    assert!(readiness.is_lio());
    assert!(readiness.contains(Readiness::READABLE | Readiness::ERROR));
    assert!(readiness.intersects(Readiness::WRITE_CLOSED));
    assert!(!readiness.contains(Readiness::WRITABLE));
    assert_eq!(
        readiness.remove(Readiness::ERROR | Readiness::AIO),
        Readiness::READABLE | Readiness::WRITE_CLOSED | Readiness::PRIORITY | Readiness::LIO,
    );
    assert_eq!(
        format!("{readiness:?}"),
        "READABLE | WRITE_CLOSED | ERROR | PRIORITY | AIO | LIO",
    );
    assert_eq!(format!("{:?}", Readiness::EMPTY), "EMPTY");
    let mut assigned = Readiness::READABLE;
    assigned |= Readiness::WRITABLE;
    assert_eq!(assigned, Readiness::READABLE | Readiness::WRITABLE);
}

#[test]
fn poll_report_accounting_is_consistent_by_construction() {
    let report = PollReport::new(5, 2, 3, true);

    assert_eq!(report.observed(), 8);
    assert_eq!(report.delivered(), 5);
    assert_eq!(report.wakes(), 2);
    assert_eq!(report.resources(), 3);
    assert_eq!(report.stale(), 3);
    assert!(report.saturated());
}

#[test]
#[should_panic(expected = "wake count exceeds delivered events")]
fn poll_report_rejects_impossible_wake_accounting() {
    let _ = PollReport::new(1, 2, 0, false);
}

#[test]
fn poll_events_are_fixed_capacity_and_ownership_preserving() {
    let token = ResourceToken::new(
        ResourceOwnerId::new(7),
        ResourceSlotId::new(3),
        ResourceGeneration::new(2),
    );
    let mut events = PollEvents::new(nonzero(1));
    events
        .try_push(PollEvent::Resource {
            token,
            readiness: Readiness::READABLE,
        })
        .unwrap_or_else(|error| panic!("first readiness must fit: {error}"));

    let error = match events.try_push(PollEvent::Wake) {
        Ok(()) => panic!("full poll batch must reject another event"),
        Err(error) => error,
    };

    assert_eq!(error.event(), PollEvent::Wake);
    assert_eq!(error.capacity(), nonzero(1));
    assert_eq!(events.len(), 1);
    assert_eq!(events.as_slice().len(), 1);
    assert_eq!(
        events.get(0).and_then(|event| event.resource()),
        Some(token)
    );
    assert_eq!((&events).into_iter().count(), 1);
    assert_eq!(events.drain().collect::<Vec<_>>().len(), 1);
    assert!(events.is_empty());
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
