//! Bounded shutdown registration, retry, terminal, and race scenarios.

use std::{
    cell::Cell,
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use calandria::{CompletionError, ShutdownSubscribeError, shutdown_barrier};

#[test]
fn one_request_admits_every_bounded_subscriber() {
    let (requester, mut completer) = shutdown_barrier(nonzero(2));
    let requests = Cell::new(0);
    let first = requester
        .subscribe(|| {
            requests.set(requests.get() + 1);
            Ok::<_, ()>(())
        })
        .unwrap_or_else(|_| panic!("admit first shutdown observer"));
    let second = requester
        .subscribe(|| {
            requests.set(requests.get() + 1);
            Ok::<_, ()>(())
        })
        .unwrap_or_else(|_| panic!("admit second shutdown observer"));

    completer.complete();

    assert_eq!(requests.get(), 1);
    assert_eq!(first.wait(), Ok(()));
    assert_eq!(second.wait(), Ok(()));
}

#[test]
fn subscriber_capacity_rejects_without_publishing_an_extra_request() {
    let (requester, _completer) = shutdown_barrier(NonZeroUsize::MIN);
    let first = requester.subscribe(|| Ok::<_, ()>(()));
    let second = requester.subscribe(|| -> Result<(), ()> {
        panic!("a full follower must not publish shutdown")
    });

    assert!(first.is_ok());
    assert!(matches!(second, Err(ShutdownSubscribeError::Full)));
}

#[test]
fn failed_first_request_reopens_the_barrier() {
    let (requester, mut completer) = shutdown_barrier(NonZeroUsize::MIN);
    let failed = requester.subscribe(|| Err("wake failed"));
    assert!(matches!(
        failed,
        Err(ShutdownSubscribeError::Request("wake failed"))
    ));
    let retried = requester
        .subscribe(|| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("retry first shutdown request"));

    completer.complete();

    assert_eq!(retried.wait(), Ok(()));
}

#[test]
fn panicking_first_request_reopens_the_barrier() {
    let (requester, mut completer) = shutdown_barrier(NonZeroUsize::MIN);
    let panicked = catch_unwind(AssertUnwindSafe(|| {
        let _ = requester.subscribe(|| -> Result<(), ()> {
            panic!("request publication panicked")
        });
    }));
    assert!(panicked.is_err());
    let retried = requester
        .subscribe(|| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("retry after request panic"));

    completer.complete();

    assert_eq!(retried.wait(), Ok(()));
}

#[test]
fn a_waiting_follower_retries_after_the_first_request_fails() {
    for _ in 0..64 {
        let (requester, mut completer) = shutdown_barrier(NonZeroUsize::MIN);
        let requester = Arc::new(requester);
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let leader_requester = Arc::clone(&requester);
        let leader_entered = Arc::clone(&entered);
        let leader_release = Arc::clone(&release);
        let leader = thread::spawn(move || {
            leader_requester.subscribe(|| {
                leader_entered.wait();
                leader_release.wait();
                Err("first request failed")
            })
        });

        entered.wait();
        let retries = Arc::new(AtomicUsize::new(0));
        let follower_requester = Arc::clone(&requester);
        let follower_retries = Arc::clone(&retries);
        let follower = thread::spawn(move || {
            follower_requester.subscribe(|| {
                follower_retries.fetch_add(1, Ordering::SeqCst);
                Ok::<_, ()>(())
            })
        });
        thread::yield_now();
        release.wait();

        assert!(matches!(
            leader.join().unwrap_or_else(|_| panic!("leader panicked")),
            Err(ShutdownSubscribeError::Request("first request failed"))
        ));
        let completion = follower
            .join()
            .unwrap_or_else(|_| panic!("follower panicked"))
            .unwrap_or_else(|_| panic!("follower retry must succeed"));
        completer.complete();

        assert_eq!(retries.load(Ordering::SeqCst), 1);
        assert_eq!(completion.wait(), Ok(()));
    }
}

#[test]
fn explicit_terminal_close_closes_all_observers() {
    let (requester, completer) = shutdown_barrier(nonzero(2));
    let first = requester
        .subscribe(|| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("admit first shutdown observer"));
    let second = requester
        .subscribe(|| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("admit second shutdown observer"));

    completer.close();

    assert_eq!(first.wait(), Err(CompletionError::Closed));
    assert_eq!(second.wait(), Err(CompletionError::Closed));
    assert!(matches!(
        requester.subscribe(|| Ok::<_, ()>(())),
        Err(ShutdownSubscribeError::Closed)
    ));
}

#[test]
fn dropping_terminal_ownership_closes_all_observers() {
    let (requester, completer) = shutdown_barrier(nonzero(2));
    let first = requester
        .subscribe(|| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("admit first shutdown observer"));
    let second = requester
        .subscribe(|| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("admit second shutdown observer"));

    drop(completer);

    assert_eq!(first.wait(), Err(CompletionError::Closed));
    assert_eq!(second.wait(), Err(CompletionError::Closed));
    assert!(matches!(
        requester.subscribe(|| Ok::<_, ()>(())),
        Err(ShutdownSubscribeError::Closed)
    ));
}

#[test]
fn completed_barrier_accepts_late_observers_without_requesting_again() {
    let (requester, mut completer) = shutdown_barrier(NonZeroUsize::MIN);
    completer.complete();
    completer.complete();
    assert!(completer.is_settled());

    let late = requester
        .subscribe(|| -> Result<(), ()> { panic!("terminal barrier must not request again") })
        .unwrap_or_else(|_| panic!("observe completed shutdown"));

    assert_eq!(late.wait(), Ok(()));
}

#[test]
fn concurrent_subscribers_publish_one_request() {
    let (requester, mut completer) = shutdown_barrier(nonzero(2));
    let requester = Arc::new(requester);
    let gate = Arc::new(Barrier::new(3));
    let requests = Arc::new(AtomicUsize::new(0));

    let first = spawn_subscriber(&requester, &gate, &requests);
    let second = spawn_subscriber(&requester, &gate, &requests);
    gate.wait();

    let first = first
        .join()
        .unwrap_or_else(|_| panic!("first subscriber panicked"));
    let second = second
        .join()
        .unwrap_or_else(|_| panic!("second subscriber panicked"));
    completer.complete();

    assert_eq!(requests.load(Ordering::SeqCst), 1);
    assert_eq!(first.wait(), Ok(()));
    assert_eq!(second.wait(), Ok(()));
}

#[test]
fn completion_during_request_publication_wins_the_race() {
    let (requester, mut completer) = shutdown_barrier(NonZeroUsize::MIN);
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let thread_entered = Arc::clone(&entered);
    let thread_release = Arc::clone(&release);
    let subscriber = thread::spawn(move || {
        requester.subscribe(|| {
            thread_entered.wait();
            thread_release.wait();
            Ok::<_, ()>(())
        })
    });

    entered.wait();
    completer.complete();
    release.wait();

    let completion = subscriber
        .join()
        .unwrap_or_else(|_| panic!("subscriber panicked"))
        .unwrap_or_else(|_| panic!("completed barrier must admit observer"));
    assert_eq!(completion.wait(), Ok(()));
}

#[test]
fn closure_during_request_publication_wins_the_race() {
    let (requester, completer) = shutdown_barrier(NonZeroUsize::MIN);
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let thread_entered = Arc::clone(&entered);
    let thread_release = Arc::clone(&release);
    let subscriber = thread::spawn(move || {
        requester.subscribe(|| {
            thread_entered.wait();
            thread_release.wait();
            Ok::<_, ()>(())
        })
    });

    entered.wait();
    drop(completer);
    release.wait();

    let result = subscriber
        .join()
        .unwrap_or_else(|_| panic!("subscriber panicked"));
    assert!(matches!(result, Err(ShutdownSubscribeError::Closed)));
}

#[test]
fn terminal_completion_supersedes_a_racing_request_failure() {
    let (requester, mut completer) = shutdown_barrier(NonZeroUsize::MIN);
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let thread_entered = Arc::clone(&entered);
    let thread_release = Arc::clone(&release);
    let subscriber = thread::spawn(move || {
        requester.subscribe(|| {
            thread_entered.wait();
            thread_release.wait();
            Err("request lost")
        })
    });

    entered.wait();
    completer.complete();
    release.wait();

    let completion = subscriber
        .join()
        .unwrap_or_else(|_| panic!("subscriber panicked"))
        .unwrap_or_else(|_| panic!("terminal completion must supersede request failure"));
    assert_eq!(completion.wait(), Ok(()));
}

fn spawn_subscriber(
    requester: &Arc<calandria::ShutdownRequester>,
    gate: &Arc<Barrier>,
    requests: &Arc<AtomicUsize>,
) -> thread::JoinHandle<calandria::Completion<()>> {
    let requester = Arc::clone(requester);
    let gate = Arc::clone(gate);
    let requests = Arc::clone(requests);
    thread::spawn(move || {
        gate.wait();
        requester
            .subscribe(|| {
                requests.fetch_add(1, Ordering::SeqCst);
                Ok::<_, ()>(())
            })
            .unwrap_or_else(|_| panic!("admit concurrent shutdown observer"))
    })
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test capacity must be nonzero"))
}
