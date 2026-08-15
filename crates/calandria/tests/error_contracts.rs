//! Public recovery-error ownership, identity, display, and source contracts.

use std::{error::Error, fmt, io, num::NonZeroUsize};

use calandria::{
    AdmissionFailure, Deadline, EventBatch, EventBatchFailure, EventBatchLimits, HostError,
    HostPhase, Lane, LaneLimits, MailboxLimits, Moment, ResourceAdmissionFailure,
    ResourceGeneration, ResourceOwnerId, ResourceSlotId, ResourceTable, ResourceTokenFailure,
    Retained, RetainedBytes, TimerLimits, TimerOwnerId, TimerQueue, TimerScheduleFailure,
    WakeHandle, mailbox,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Planned(&'static str);

impl fmt::Display for Planned {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for Planned {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
fn host_failures_distinguish_terminal_phase_and_owned_sources() {
    let terminal: HostError<Planned, Planned> = HostError::NotRunning {
        phase: HostPhase::Terminated,
    };
    assert_eq!(terminal.phase(), Some(HostPhase::Terminated));
    assert_eq!(terminal.to_string(), "host is already Terminated");
    assert!(Error::source(&terminal).is_none());

    let clock: HostError<Planned, Planned> = HostError::Clock(Planned("clock source"));
    assert_eq!(clock.phase(), None);
    assert_eq!(clock.to_string(), "host clock failed: clock source");
    assert_eq!(source_text(&clock), Some(String::from("clock source")));

    let regression: HostError<Planned, Planned> = HostError::ClockRegressed {
        previous: Moment::from_nanos(8),
        observed: Moment::from_nanos(7),
    };
    assert_eq!(regression.phase(), None);
    assert_eq!(
        regression.to_string(),
        "host clock regressed from 8ns to 7ns"
    );
    assert!(Error::source(&regression).is_none());

    let duty: HostError<Planned, Planned> = HostError::Duty(Planned("duty source"));
    assert_eq!(duty.phase(), None);
    assert_eq!(duty.to_string(), "host duty failed: duty source");
    assert_eq!(source_text(&duty), Some(String::from("duty source")));
}

#[test]
fn event_batch_error_splits_exact_owner_and_failure() {
    let mut batch = EventBatch::new(EventBatchLimits::new(nonzero(1), RetainedBytes::new(8)));
    batch
        .try_push(owned(1, 1))
        .unwrap_or_else(|error| panic!("first event must fit: {error}"));
    let Err(error) = batch.try_push(owned(2, 2)) else {
        panic!("second event must exceed count");
    };
    assert_eq!(error.to_string(), "event batch capacity of 1 was reached");
    assert!(Error::source(&error).is_some());
    assert_eq!(
        error.into_parts(),
        (
            owned(2, 2),
            EventBatchFailure::EventCapacity { limit: nonzero(1) }
        )
    );

    assert_eq!(
        EventBatchFailure::RetainedByteOverflow {
            current: RetainedBytes::new(u64::MAX),
            event: RetainedBytes::new(1),
        }
        .to_string(),
        "adding 1 retained bytes to 18446744073709551615 would overflow event-batch accounting"
    );
    assert_eq!(
        EventBatchFailure::RetainedByteCapacity {
            limit: RetainedBytes::new(3),
            current: RetainedBytes::new(2),
            event: RetainedBytes::new(2),
        }
        .to_string(),
        "adding 2 retained bytes to 2 would exceed event-batch limit 3"
    );
}

#[test]
fn timer_error_splits_deadline_value_and_failure() {
    let limits = TimerLimits::new(nonzero(1), RetainedBytes::new(8));
    let mut timers = TimerQueue::new(TimerOwnerId::new(4), limits);
    timers
        .schedule(deadline(5), owned(1, 1))
        .unwrap_or_else(|error| panic!("first timer must fit: {error}"));
    let Err(error) = timers.schedule(deadline(6), owned(2, 2)) else {
        panic!("second timer must exceed count");
    };
    assert_eq!(error.to_string(), "timer capacity of 1 was reached");
    assert!(Error::source(&error).is_some());
    assert_eq!(
        error.into_parts(),
        (
            deadline(6),
            owned(2, 2),
            TimerScheduleFailure::TimerCapacity { limit: nonzero(1) }
        )
    );

    let cases = [
        (
            TimerScheduleFailure::RetainedByteOverflow {
                current: RetainedBytes::new(u64::MAX),
                timer: RetainedBytes::new(1),
            },
            "adding 1 retained bytes to 18446744073709551615 would overflow timer accounting",
        ),
        (
            TimerScheduleFailure::RetainedByteCapacity {
                limit: RetainedBytes::new(3),
                current: RetainedBytes::new(2),
                timer: RetainedBytes::new(2),
            },
            "adding 2 retained bytes to 2 would exceed timer limit 3",
        ),
        (
            TimerScheduleFailure::TimerIdsExhausted,
            "timer identities are exhausted",
        ),
    ];
    for (failure, expected) in cases {
        assert_eq!(failure.to_string(), expected);
    }
}

#[test]
fn mailbox_error_preserves_lane_owner_and_nested_wake_source() {
    let limits = MailboxLimits::new(
        LaneLimits::new(nonzero(1), RetainedBytes::new(8)),
        LaneLimits::new(nonzero(1), RetainedBytes::new(8)),
    );
    let wake = WakeHandle::new(|| Ok(()));
    let (sender, _receiver) = mailbox(limits, wake);
    sender
        .try_send(owned(1, 1))
        .unwrap_or_else(|error| panic!("first message must fit: {error}"));
    let Err(error) = sender.try_send(owned(2, 2)) else {
        panic!("second work message must exceed count");
    };
    assert_eq!(error.lane(), Lane::Work);
    assert_eq!(
        error.to_string(),
        "mailbox message capacity reached in Work lane"
    );
    assert!(Error::source(&error).is_some());
    let (item, lane, failure) = error.into_parts();
    assert_eq!((item, lane), (owned(2, 2), Lane::Work));
    assert!(matches!(failure, AdmissionFailure::MessageCapacity));

    let cases = [
        (
            AdmissionFailure::ByteCapacity,
            "mailbox retained-byte capacity reached",
        ),
        (AdmissionFailure::Closed, "mailbox receiver is closed"),
    ];
    for (failure, expected) in cases {
        assert!(failure.wake_error().is_none());
        assert!(Error::source(&failure).is_none());
        assert_eq!(failure.to_string(), expected);
    }

    let wake = AdmissionFailure::Wake(io::Error::other("planned wake"));
    assert_eq!(
        wake.wake_error().map(io::Error::kind),
        Some(io::ErrorKind::Other)
    );
    assert_eq!(source_text(&wake), Some(String::from("planned wake")));
    assert_eq!(wake.to_string(), "mailbox wake failed: planned wake");
}

#[test]
fn resource_failures_preserve_values_and_fenced_identities() {
    let mut table = ResourceTable::new(ResourceOwnerId::new(7), nonzero(1));
    table
        .admit("same", "kept")
        .unwrap_or_else(|error| panic!("first resource must fit: {error}"));
    let Err(error) = table.admit("same", "returned") else {
        panic!("duplicate identity must reject");
    };
    assert_eq!(error.to_string(), "resource identity is already admitted");
    assert!(Error::source(&error).is_some());
    assert_eq!(
        error.into_parts(),
        ("same", "returned", ResourceAdmissionFailure::IdentityInUse)
    );

    let admission_cases = [
        (
            ResourceAdmissionFailure::CapacityReached { limit: nonzero(1) },
            "resource capacity of 1 was reached",
        ),
        (
            ResourceAdmissionFailure::TokenSpaceExhausted,
            "resource token generations are exhausted",
        ),
    ];
    for (failure, expected) in admission_cases {
        assert_eq!(failure.to_string(), expected);
    }

    let slot = ResourceSlotId::new(3);
    let token_cases = [
        (
            ResourceTokenFailure::OwnerMismatch {
                expected: ResourceOwnerId::new(1),
                actual: ResourceOwnerId::new(2),
            },
            "resource token owner 2 does not match owner 1",
        ),
        (
            ResourceTokenFailure::SlotOutOfBounds {
                slot,
                capacity: nonzero(2),
            },
            "resource token slot 3 exceeds capacity 2",
        ),
        (
            ResourceTokenFailure::Vacant {
                slot,
                generation: ResourceGeneration::new(4),
            },
            "resource slot 3 is vacant at generation 4",
        ),
        (
            ResourceTokenFailure::GenerationMismatch {
                slot,
                current: ResourceGeneration::new(4),
                supplied: ResourceGeneration::new(2),
            },
            "resource slot 3 is at generation 4, not 2",
        ),
        (
            ResourceTokenFailure::Exhausted { slot },
            "resource slot 3 is exhausted",
        ),
    ];
    for (failure, expected) in token_cases {
        assert_eq!(failure.to_string(), expected);
    }
}

fn source_text(error: &(impl Error + 'static)) -> Option<String> {
    Error::source(error).map(ToString::to_string)
}

const fn owned(id: u8, bytes: u64) -> Owned {
    Owned {
        id,
        bytes: RetainedBytes::new(bytes),
    }
}

const fn deadline(at: u64) -> Deadline {
    Deadline::at(Moment::from_nanos(at))
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
