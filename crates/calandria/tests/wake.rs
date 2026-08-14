//! Coalesced wake linearization and failure-retry tests.

use std::{
    io,
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use calandria::{WakeHandle, WakeSource};

#[derive(Clone, Debug)]
struct CountingWake {
    calls: Arc<AtomicUsize>,
    fail_first: bool,
}

impl WakeSource for CountingWake {
    fn wake(&self) -> io::Result<()> {
        let call = self.calls.fetch_add(1, Ordering::Relaxed);
        if self.fail_first && call == 0 {
            Err(io::Error::other("planned wake failure"))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
struct BlockingWake {
    calls: Arc<AtomicUsize>,
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    fail_first: bool,
}

impl WakeSource for BlockingWake {
    fn wake(&self) -> io::Result<()> {
        let call = self.calls.fetch_add(1, Ordering::Relaxed);
        if call == 0 {
            self.entered.wait();
            self.release.wait();
            if self.fail_first {
                return Err(io::Error::other("planned blocking wake failure"));
            }
        }
        Ok(())
    }
}

#[test]
fn requests_coalesce_until_acknowledged() {
    let calls = Arc::new(AtomicUsize::new(0));
    let wake = WakeHandle::new(CountingWake {
        calls: Arc::clone(&calls),
        fail_first: false,
    });

    assert!(wake.wake().is_ok());
    assert!(wake.wake().is_ok());
    assert_eq!(calls.load(Ordering::Relaxed), 1);

    wake.acknowledge();
    assert!(wake.wake().is_ok());
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn failed_backend_request_can_be_retried() {
    let calls = Arc::new(AtomicUsize::new(0));
    let wake = WakeHandle::new(CountingWake {
        calls: Arc::clone(&calls),
        fail_first: true,
    });

    assert!(wake.wake().is_err());
    assert!(!wake.is_requested());
    assert!(wake.wake().is_ok());
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn acknowledgement_during_backend_wake_is_not_lost() {
    let calls = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let wake = WakeHandle::new(BlockingWake {
        calls: Arc::clone(&calls),
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
        fail_first: false,
    });
    let worker_wake = wake.clone();
    let worker = thread::spawn(move || worker_wake.wake());

    entered.wait();
    wake.acknowledge();
    release.wait();

    let result = worker
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    assert!(result.is_ok());
    assert!(!wake.is_requested());

    assert!(wake.wake().is_ok());
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn concurrent_callers_do_not_observe_failed_wake_as_success() {
    let calls = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let wake = WakeHandle::new(BlockingWake {
        calls: Arc::clone(&calls),
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
        fail_first: true,
    });

    let first_wake = wake.clone();
    let first = thread::spawn(move || first_wake.wake());
    entered.wait();

    let second_wake = wake.clone();
    let second = thread::spawn(move || second_wake.wake());
    release.wait();

    let first_result = first
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    let second_result = second
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    assert!(first_result.is_err());
    assert!(second_result.is_ok());
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert!(wake.is_requested());
}
