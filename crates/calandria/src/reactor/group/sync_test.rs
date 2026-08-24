//! Loom schedules over production group admission and startup fences.

use std::num::NonZeroUsize;

use loom::thread;

use crate::{DrainStatus, LaneLimits, MailboxLimits, RetainedBytes, WakeHandle, mailbox_with};

use super::{ReactorId, StartGate, bounded_ingress};

#[test]
fn group_close_fences_each_production_shard_admission() {
    for target in 0..2 {
        loom::model(move || {
            let (first_sender, mut first_receiver) = mailbox();
            let (second_sender, mut second_receiver) = mailbox();
            let (handle, closer) = bounded_ingress(vec![first_sender, second_sender]);
            let sender = handle.clone();
            let publish = thread::spawn(move || {
                let value = u8::try_from(target)
                    .unwrap_or_else(|_| panic!("test shard identity must fit in one byte"));
                sender.try_send(ReactorId::new(target), value).is_ok()
            });
            let close = thread::spawn(move || closer.close());

            let accepted = publish
                .join()
                .unwrap_or_else(|_| panic!("production group sender panicked"));
            close
                .join()
                .unwrap_or_else(|_| panic!("production group closer panicked"));
            let mut first = Vec::new();
            let mut second = Vec::new();
            let first_status = first_receiver.drain_into(&mut first, nonzero(1)).status();
            let second_status = second_receiver.drain_into(&mut second, nonzero(1)).status();

            assert!(!handle.is_open());
            assert_eq!(
                first,
                if accepted && target == 0 {
                    vec![0]
                } else {
                    vec![]
                }
            );
            assert_eq!(
                second,
                if accepted && target == 1 {
                    vec![1]
                } else {
                    vec![]
                }
            );
            assert!(matches!(
                first_status,
                DrainStatus::Idle | DrainStatus::Closed
            ));
            assert!(matches!(
                second_status,
                DrainStatus::Idle | DrainStatus::Closed
            ));
        });
    }
}

#[test]
fn reactor_group_start_decision_uses_the_production_gate_once() {
    loom::model(|| {
        let gate = StartGate::new();
        let waiter = gate.clone();
        let observed = thread::spawn(move || waiter.wait());
        let starter = gate.clone();
        let start = thread::spawn(move || starter.start());
        let aborter = gate.clone();
        let abort = thread::spawn(move || aborter.abort());

        start
            .join()
            .unwrap_or_else(|_| panic!("production group starter panicked"));
        abort
            .join()
            .unwrap_or_else(|_| panic!("production group aborter panicked"));
        let observed = observed
            .join()
            .unwrap_or_else(|_| panic!("production group waiter panicked"));

        assert_eq!(observed, gate.wait());
    });
}

fn mailbox() -> (crate::MailboxSender<u8>, crate::MailboxReceiver<u8>) {
    let lane = LaneLimits::new(nonzero(1), RetainedBytes::ZERO);
    mailbox_with(
        MailboxLimits::new(lane, lane),
        |_| RetainedBytes::ZERO,
        WakeHandle::new(|| Ok(())),
    )
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test capacity must be nonzero"))
}
