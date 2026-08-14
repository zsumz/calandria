//! Loom models for completion ownership and shutdown request linearization.

use loom::{
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompletionState {
    PendingAlive,
    PendingAbandoned,
    Ready,
    Consumed,
}

#[test]
fn completion_publication_racing_abandonment_drops_one_value() {
    loom::model(|| {
        let state = Arc::new(Mutex::new(CompletionState::PendingAlive));
        let drops = Arc::new(AtomicUsize::new(0));
        let producer_state = Arc::clone(&state);
        let producer_drops = Arc::clone(&drops);
        let producer = thread::spawn(move || {
            let mut state = lock(&producer_state);
            match *state {
                CompletionState::PendingAlive => *state = CompletionState::Ready,
                CompletionState::PendingAbandoned => {
                    *state = CompletionState::Consumed;
                    producer_drops.fetch_add(1, Ordering::SeqCst);
                }
                CompletionState::Ready | CompletionState::Consumed => {
                    panic!("completion producer observed impossible phase")
                }
            }
        });
        let observer_state = Arc::clone(&state);
        let observer_drops = Arc::clone(&drops);
        let observer = thread::spawn(move || {
            let mut state = lock(&observer_state);
            match *state {
                CompletionState::PendingAlive => *state = CompletionState::PendingAbandoned,
                CompletionState::Ready => {
                    *state = CompletionState::Consumed;
                    observer_drops.fetch_add(1, Ordering::SeqCst);
                }
                CompletionState::PendingAbandoned | CompletionState::Consumed => {
                    panic!("single observer entered an impossible phase")
                }
            }
        });

        producer
            .join()
            .unwrap_or_else(|_| panic!("modeled completion producer panicked"));
        observer
            .join()
            .unwrap_or_else(|_| panic!("modeled completion observer panicked"));
        assert_eq!(*lock(&state), CompletionState::Consumed);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    });
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShutdownPhase {
    Open,
    Requesting,
    Requested,
}

#[derive(Debug)]
struct ShutdownState {
    phase: ShutdownPhase,
    subscribers: usize,
}

#[test]
fn concurrent_shutdown_subscribers_publish_one_successful_request() {
    loom::model(|| {
        let shared = shutdown_state();
        let attempts = Arc::new(AtomicUsize::new(0));
        let first = spawn_subscriber(&shared, &attempts, false);
        let second = spawn_subscriber(&shared, &attempts, false);

        assert!(join(first));
        assert!(join(second));
        let state = lock_state(&shared);
        assert_eq!(state.phase, ShutdownPhase::Requested);
        assert_eq!(state.subscribers, 2);
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn failed_shutdown_request_reopens_for_one_waiting_retry() {
    loom::model(|| {
        let shared = shutdown_state();
        let attempts = Arc::new(AtomicUsize::new(0));
        let first = spawn_subscriber(&shared, &attempts, true);
        let second = spawn_subscriber(&shared, &attempts, true);

        let first = join(first);
        let second = join(second);
        assert_ne!(first, second);
        let state = lock_state(&shared);
        assert_eq!(state.phase, ShutdownPhase::Requested);
        assert_eq!(state.subscribers, 1);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    });
}

type ShutdownShared = Arc<(Mutex<ShutdownState>, Condvar)>;

fn shutdown_state() -> ShutdownShared {
    Arc::new((
        Mutex::new(ShutdownState {
            phase: ShutdownPhase::Open,
            subscribers: 0,
        }),
        Condvar::new(),
    ))
}

fn spawn_subscriber(
    shared: &ShutdownShared,
    attempts: &Arc<AtomicUsize>,
    fail_first: bool,
) -> thread::JoinHandle<bool> {
    let shared = Arc::clone(shared);
    let attempts = Arc::clone(attempts);
    thread::spawn(move || subscribe(&shared, &attempts, fail_first))
}

fn subscribe(shared: &ShutdownShared, attempts: &AtomicUsize, fail_first: bool) -> bool {
    loop {
        let mut state = lock_state(shared);
        match state.phase {
            ShutdownPhase::Open => {
                state.phase = ShutdownPhase::Requesting;
                drop(state);
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                thread::yield_now();
                let mut state = lock_state(shared);
                if fail_first && attempt == 0 {
                    state.phase = ShutdownPhase::Open;
                    shared.1.notify_all();
                    return false;
                }
                state.phase = ShutdownPhase::Requested;
                state.subscribers += 1;
                shared.1.notify_all();
                return true;
            }
            ShutdownPhase::Requesting => {
                drop(
                    shared
                        .1
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner),
                );
            }
            ShutdownPhase::Requested => {
                state.subscribers += 1;
                return true;
            }
        }
    }
}

fn join(handle: thread::JoinHandle<bool>) -> bool {
    handle
        .join()
        .unwrap_or_else(|_| panic!("modeled shutdown subscriber panicked"))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn lock_state(shared: &ShutdownShared) -> MutexGuard<'_, ShutdownState> {
    lock(&shared.0)
}
