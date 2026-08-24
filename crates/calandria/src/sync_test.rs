//! Loom schedules over production wake, mailbox, completion, and shutdown code.

use std::num::NonZeroUsize;

use loom::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use crate::{
    DrainStatus, LaneLimits, MailboxLimits, RetainedBytes, ShutdownSubscribeError, WakeHandle,
    completion, mailbox_with, shutdown_barrier,
};

#[test]
fn wake_request_racing_owner_acknowledgement_rearms_production_state() {
    loom::model(|| {
        let calls = Arc::new(AtomicUsize::new(0));
        let source_calls = Arc::clone(&calls);
        let wake = WakeHandle::new(move || {
            source_calls.fetch_add(1, Ordering::SeqCst);
            thread::yield_now();
            Ok(())
        });
        let requester = wake.clone();
        let request = thread::spawn(move || requester.wake());
        let owner = wake.clone();
        let owner_calls = Arc::clone(&calls);
        let acknowledgement = thread::spawn(move || {
            while owner_calls.load(Ordering::SeqCst) == 0 {
                thread::yield_now();
            }
            owner.acknowledge();
        });

        assert!(join(request).is_ok());
        join(acknowledgement);
        assert!(wake.wake().is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        wake.acknowledge();
        assert!(!wake.is_requested());
    });
}

#[test]
fn mailbox_publication_racing_empty_acknowledgement_is_not_stranded() {
    loom::model(|| {
        let wake = WakeHandle::new(|| Ok(()));
        let (sender, mut receiver) = mailbox_with(limits(), |_| RetainedBytes::ZERO, wake.clone());
        let publish = thread::spawn(move || sender.try_send(7_u8));
        let drain = thread::spawn(move || {
            let mut values = Vec::new();
            let report = receiver.drain_into(&mut values, nonzero(1));
            (receiver, values, report.status())
        });

        assert!(join(publish).is_ok());
        let (mut receiver, mut values, first_status) = join(drain);
        let final_report = receiver.drain_into(&mut values, nonzero(1));

        assert_eq!(values, vec![7]);
        assert!(matches!(
            first_status,
            DrainStatus::Idle | DrainStatus::Closed
        ));
        assert_eq!(final_report.status(), DrainStatus::Closed);
        assert!(!wake.is_requested());
    });
}

#[test]
fn completion_publication_racing_abandonment_drops_one_value() {
    loom::model(|| {
        let drops = Arc::new(AtomicUsize::new(0));
        let (completion, completer) = completion();
        let value = DropCount(Arc::clone(&drops));
        let publish = thread::spawn(move || drop(completer.complete(value)));
        let abandon = thread::spawn(move || drop(completion));

        join(publish);
        join(abandon);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn completion_publication_racing_extraction_returns_the_value_once() {
    loom::model(|| {
        let (completion, completer) = completion();
        let publish = thread::spawn(move || completer.complete(9_u8));
        let extract = thread::spawn(move || {
            loop {
                if let Some(result) = completion.try_take() {
                    return result;
                }
                thread::yield_now();
            }
        });

        assert_eq!(join(publish), Ok(()));
        assert_eq!(join(extract), Ok(9));
    });
}

#[test]
fn concurrent_shutdown_subscribers_publish_one_production_request() {
    loom::model(|| {
        let (requester, mut completer) = shutdown_barrier(nonzero(2));
        let attempts = Arc::new(AtomicUsize::new(0));
        let first = spawn_subscriber(requester.clone(), Arc::clone(&attempts), false);
        let second = spawn_subscriber(requester, Arc::clone(&attempts), false);

        let first = join(first).unwrap_or_else(|_| panic!("first subscriber must fit"));
        let second = join(second).unwrap_or_else(|_| panic!("second subscriber must fit"));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        completer.complete();
        assert_eq!(first.try_take(), Some(Ok(())));
        assert_eq!(second.try_take(), Some(Ok(())));
    });
}

#[test]
fn failed_shutdown_request_reopens_for_one_production_retry() {
    loom::model(|| {
        let (requester, mut completer) = shutdown_barrier(nonzero(2));
        let attempts = Arc::new(AtomicUsize::new(0));
        let first = spawn_subscriber(requester.clone(), Arc::clone(&attempts), true);
        let second = spawn_subscriber(requester, Arc::clone(&attempts), true);

        let first = join(first);
        let second = join(second);
        let completion = match (first, second) {
            (Ok(completion), Err(ShutdownSubscribeError::Request(())))
            | (Err(ShutdownSubscribeError::Request(())), Ok(completion)) => completion,
            _ => panic!("exactly one failed request and one successful retry are required"),
        };
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        completer.complete();
        assert_eq!(completion.try_take(), Some(Ok(())));
    });
}

struct DropCount(Arc<AtomicUsize>);

impl Drop for DropCount {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn spawn_subscriber(
    requester: crate::ShutdownRequester,
    attempts: Arc<AtomicUsize>,
    fail_first: bool,
) -> thread::JoinHandle<Result<crate::Completion<()>, ShutdownSubscribeError<()>>> {
    thread::spawn(move || {
        requester.subscribe(|| {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            thread::yield_now();
            if fail_first && attempt == 0 {
                Err(())
            } else {
                Ok(())
            }
        })
    })
}

fn limits() -> MailboxLimits {
    let lane = LaneLimits::new(nonzero(1), RetainedBytes::ZERO);
    MailboxLimits::new(lane, lane)
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test capacity must be nonzero"))
}

fn join<T>(handle: thread::JoinHandle<T>) -> T {
    handle
        .join()
        .unwrap_or_else(|_| panic!("production Loom thread panicked"))
}
