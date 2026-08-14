//! Exactly-once completion observation, closure, and race scenarios.

use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    thread,
};

use calandria::{Completion, CompletionError, completion};

#[test]
fn blocking_wait_receives_the_producer_value() {
    let (completion, completer) = completion();
    let producer = thread::spawn(move || completer.complete(42));

    assert_eq!(completion.wait(), Ok(42));
    assert!(matches!(producer.join(), Ok(Ok(()))));
}

#[test]
fn explicit_producer_close_releases_a_blocking_waiter() {
    let (completion, completer) = completion::<u8>();
    completer.close();

    assert_eq!(completion.wait(), Err(CompletionError::Closed));
}

#[test]
fn producer_drop_releases_a_blocking_waiter() {
    let (completion, completer) = completion::<u8>();
    drop(completer);

    assert_eq!(completion.wait(), Err(CompletionError::Closed));
}

#[test]
fn explicit_abandonment_returns_an_undelivered_value() {
    let (completion, completer) = completion::<&str>();
    completion.abandon();

    assert_eq!(completer.complete("unobserved"), Err("unobserved"));
}

#[test]
fn observer_drop_returns_an_undelivered_value() {
    let (completion, completer) = completion::<&str>();
    drop(completion);

    assert_eq!(completer.complete("unobserved"), Err("unobserved"));
}

#[test]
fn nonblocking_observation_is_pending_then_consumes_once() {
    let (mut completion, completer) = completion();

    assert!(completion.try_take().is_none());
    assert_eq!(completer.complete(7), Ok(()));
    assert_eq!(completion.try_take(), Some(Ok(7)));
    assert_eq!(completion.try_take(), Some(Err(CompletionError::Consumed)));
}

#[test]
fn future_observation_wakes_when_the_value_arrives() {
    let (mut completion, completer) = completion::<u8>();
    let wake_count = Arc::new(WakeCount::default());
    let waker = Waker::from(Arc::clone(&wake_count));
    let mut context = Context::from_waker(&waker);

    assert_eq!(
        Future::poll(Pin::new(&mut completion), &mut context),
        Poll::Pending
    );
    assert_eq!(completer.complete(9), Ok(()));
    assert_eq!(wake_count.get(), 1);
    assert_eq!(
        Future::poll(Pin::new(&mut completion), &mut context),
        Poll::Ready(Ok(9))
    );
}

#[test]
fn repoll_replaces_a_stale_task_waker() {
    let (mut completion, completer) = completion::<u8>();
    let first = Arc::new(WakeCount::default());
    let second = Arc::new(WakeCount::default());
    let first_waker = Waker::from(Arc::clone(&first));
    let second_waker = Waker::from(Arc::clone(&second));

    assert_eq!(poll(&mut completion, &first_waker), Poll::Pending);
    assert_eq!(poll(&mut completion, &second_waker), Poll::Pending);
    assert_eq!(completer.complete(1), Ok(()));

    assert_eq!(first.get(), 0);
    assert_eq!(second.get(), 1);
}

#[test]
fn polling_after_terminal_consumption_reports_misuse() {
    let (mut completion, completer) = completion::<u8>();
    assert_eq!(completer.complete(7), Ok(()));
    let waker = Waker::from(Arc::new(WakeCount::default()));

    assert_eq!(poll(&mut completion, &waker), Poll::Ready(Ok(7)));
    assert_eq!(
        poll(&mut completion, &waker),
        Poll::Ready(Err(CompletionError::Consumed))
    );
}

#[test]
fn publication_racing_abandonment_drops_the_value_exactly_once() {
    for _ in 0..256 {
        let (completion, completer) = completion();
        let gate = Arc::new(Barrier::new(2));
        let drops = Arc::new(AtomicUsize::new(0));
        let producer_gate = Arc::clone(&gate);
        let producer_drops = Arc::clone(&drops);
        let producer = thread::spawn(move || {
            producer_gate.wait();
            completer.complete(DropProbe(producer_drops))
        });

        gate.wait();
        completion.abandon();
        drop(
            producer
                .join()
                .unwrap_or_else(|_| panic!("producer panicked")),
        );

        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn completion_observer_is_send() {
    assert_send::<Completion<u8>>();
}

fn poll<T>(completion: &mut Completion<T>, waker: &Waker) -> Poll<Result<T, CompletionError>> {
    let mut context = Context::from_waker(waker);
    Future::poll(Pin::new(completion), &mut context)
}

fn assert_send<T: Send>() {}

#[derive(Debug)]
struct DropProbe(Arc<AtomicUsize>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Debug, Default)]
struct WakeCount {
    count: AtomicUsize,
}

impl WakeCount {
    fn get(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}
