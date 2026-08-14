//! Bounded mailbox admission, ownership, ordering, and wake tests.

use std::{
    io,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use calandria::{
    AdmissionFailure, DrainStatus, Lane, LaneLimits, MailboxLimits, MailboxReceiver, MailboxSender,
    Retained, RetainedBytes, WakeHandle, WakeSource, mailbox,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Message {
    id: u8,
    bytes: u64,
}

impl Retained for Message {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::new(self.bytes)
    }
}

#[derive(Clone, Debug)]
struct RecordingWake {
    calls: Arc<AtomicUsize>,
    fail: bool,
}

impl WakeSource for RecordingWake {
    fn wake(&self) -> io::Result<()> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.fail {
            Err(io::Error::other("planned wake failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn control_is_drained_first_without_reordering_either_lane() {
    let (sender, mut receiver, _) = fixture(false);
    assert!(sender.try_send(message(1, 1)).is_ok());
    assert!(sender.try_send_control(message(2, 1)).is_ok());
    assert!(sender.try_send(message(3, 1)).is_ok());
    assert!(sender.try_send_control(message(4, 1)).is_ok());

    let mut drained = Vec::new();
    let report = receiver.drain_into(&mut drained, nonzero_usize(3));

    assert_eq!(drained, [message(2, 1), message(4, 1), message(1, 1)]);
    assert_eq!(report.drained(), 3);
    assert_eq!(report.status(), DrainStatus::MorePending);
}

#[test]
fn count_and_byte_limits_are_independent_per_lane() {
    let (sender, receiver, _) = fixture(false);
    assert!(sender.try_send(message(1, 4)).is_ok());
    assert!(sender.try_send(message(2, 4)).is_ok());

    let count = sender.try_send(message(3, 1));
    assert!(matches!(
        count,
        Err(ref error) if matches!(error.failure(), AdmissionFailure::MessageCapacity)
    ));

    assert!(sender.try_send_control(message(4, 8)).is_ok());
    let bytes = sender.try_send_control(message(5, 1));
    assert!(matches!(
        bytes,
        Err(ref error) if matches!(error.failure(), AdmissionFailure::ByteCapacity)
    ));

    let snapshot = receiver.snapshot();
    assert_eq!(snapshot.lane(Lane::Work).queued_messages(), 2);
    assert_eq!(
        snapshot.lane(Lane::Control).retained_bytes(),
        RetainedBytes::new(8)
    );
    assert_eq!(snapshot.lane(Lane::Work).message_rejections(), 1);
    assert_eq!(snapshot.lane(Lane::Control).byte_rejections(), 1);
}

#[test]
fn draining_restores_exact_retained_capacity() {
    let (sender, mut receiver, _) = fixture(false);
    assert!(sender.try_send(message(1, 8)).is_ok());
    assert!(sender.try_send(message(2, 8)).is_ok());

    let mut drained = Vec::new();
    let report = receiver.drain_into(&mut drained, nonzero_usize(1));
    assert_eq!(report.status(), DrainStatus::MorePending);
    assert_eq!(drained, [message(1, 8)]);

    assert!(sender.try_send(message(3, 8)).is_ok());
    assert_eq!(
        sender.snapshot().lane(Lane::Work).retained_bytes(),
        RetainedBytes::new(16)
    );
}

#[test]
fn failed_wake_returns_ownership_without_publication() {
    let (sender, receiver, calls) = fixture(true);
    let rejected = sender.try_send(message(9, 2));

    let error = match rejected {
        Ok(()) => panic!("planned wake failure must reject admission"),
        Err(error) => error,
    };
    assert!(matches!(error.failure(), AdmissionFailure::Wake(_)));
    assert_eq!(error.into_item(), message(9, 2));
    assert_eq!(receiver.snapshot().lane(Lane::Work).queued_messages(), 0);
    assert_eq!(receiver.snapshot().wake_failures(), 1);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn wake_requests_coalesce_and_rearm_after_empty_drain() {
    let (sender, mut receiver, calls) = fixture(false);
    assert!(sender.try_send(message(1, 1)).is_ok());
    assert!(sender.try_send(message(2, 1)).is_ok());
    assert_eq!(calls.load(Ordering::Relaxed), 1);

    let mut drained = Vec::new();
    let report = receiver.drain_into(&mut drained, nonzero_usize(8));
    assert_eq!(report.status(), DrainStatus::Idle);

    assert!(sender.try_send(message(3, 1)).is_ok());
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn last_sender_wakes_receiver_to_observe_terminal_closure() {
    let (sender, mut receiver, calls) = fixture(false);
    drop(sender);

    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let report = receiver.drain_into(&mut Vec::new(), nonzero_usize(1));
    assert_eq!(report.status(), DrainStatus::Closed);
}

#[test]
fn receiver_close_returns_every_owned_value_and_rejects_later_work() {
    let (sender, mut receiver, _) = fixture(false);
    assert!(sender.try_send(message(1, 1)).is_ok());
    assert!(sender.try_send_control(message(2, 1)).is_ok());

    assert_eq!(receiver.close(), [message(2, 1), message(1, 1)]);
    let report = receiver.drain_into(&mut Vec::new(), nonzero_usize(1));
    assert_eq!(report.status(), DrainStatus::Closed);

    let error = match sender.try_send(message(3, 1)) {
        Ok(()) => panic!("closed receiver must reject admission"),
        Err(error) => error,
    };
    assert!(matches!(error.failure(), AdmissionFailure::Closed));
    assert_eq!(sender.snapshot().closed_rejections(), 1);
}

#[test]
fn zero_byte_lane_accepts_only_fixed_size_messages() {
    let calls = Arc::new(AtomicUsize::new(0));
    let wake = WakeHandle::new(RecordingWake { calls, fail: false });
    let limits = MailboxLimits::new(
        LaneLimits::new(nonzero_usize(1), RetainedBytes::ZERO),
        LaneLimits::new(nonzero_usize(2), RetainedBytes::ZERO),
    );
    let (sender, _receiver) = mailbox(limits, wake);

    assert!(sender.try_send(message(1, 0)).is_ok());
    let error = match sender.try_send(message(2, 1)) {
        Ok(()) => panic!("retained message must be rejected"),
        Err(error) => error,
    };
    assert!(matches!(error.failure(), AdmissionFailure::ByteCapacity));
    assert_eq!(error.into_item(), message(2, 1));
}

#[test]
fn materialization_occurs_only_after_admission_is_proven() {
    let (sender, _receiver, _) = fixture(false);
    assert!(sender.try_send(message(1, 8)).is_ok());
    assert!(sender.try_send(message(2, 8)).is_ok());
    let materialized = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&materialized);

    let result = sender.try_send_materialized(
        Lane::Work,
        7_u8,
        |_| RetainedBytes::new(1),
        move |id| {
            observed.fetch_add(1, Ordering::Relaxed);
            message(id, 1)
        },
    );

    assert!(matches!(
        result,
        Err(ref error) if matches!(error.failure(), AdmissionFailure::MessageCapacity)
    ));
    assert_eq!(materialized.load(Ordering::Relaxed), 0);
}

fn fixture(
    fail_wake: bool,
) -> (
    MailboxSender<Message>,
    MailboxReceiver<Message>,
    Arc<AtomicUsize>,
) {
    let calls = Arc::new(AtomicUsize::new(0));
    let wake = WakeHandle::new(RecordingWake {
        calls: Arc::clone(&calls),
        fail: fail_wake,
    });
    let limits = MailboxLimits::new(
        LaneLimits::new(nonzero_usize(2), RetainedBytes::new(8)),
        LaneLimits::new(nonzero_usize(2), RetainedBytes::new(16)),
    );
    let (sender, receiver) = mailbox(limits, wake);
    (sender, receiver, calls)
}

const fn message(id: u8, bytes: u64) -> Message {
    Message { id, bytes }
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
