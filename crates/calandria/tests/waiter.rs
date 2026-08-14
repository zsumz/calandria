use std::{
    error::Error,
    sync::mpsc,
    thread,
};

use calandria::{Span, WaitOutcome, Waiter, thread_parker};

#[test]
fn notification_before_wait_is_not_lost() {
    let (mut waiter, notifier) = thread_parker();
    notifier.notify();
    let mut duty = ();

    let outcome = match waiter.wait(&mut duty, Span::from_nanos(1_000_000_000)) {
        Ok(outcome) => outcome,
        Err(never) => match never {},
    };

    assert_eq!(outcome, WaitOutcome::Notified);
}

#[test]
fn zero_wait_returns_without_blocking() {
    let (mut waiter, _notifier) = thread_parker();
    let mut duty = ();
    let outcome = match waiter.wait(&mut duty, Span::ZERO) {
        Ok(outcome) => outcome,
        Err(never) => match never {},
    };

    assert_eq!(outcome, WaitOutcome::Idle);
}

#[test]
fn notifier_mints_independent_publication_domains() -> Result<(), Box<dyn Error>> {
    let (_waiter, notifier) = thread_parker();
    let first = notifier.wake_handle();
    let second = notifier.wake_handle();

    first.wake()?;
    second.wake()?;
    first.acknowledge();

    assert!(!first.is_requested());
    assert!(second.is_requested());
    second.acknowledge();
    Ok(())
}

#[test]
fn blocked_owner_observes_cross_thread_notification() -> Result<(), Box<dyn Error>> {
    let (mut waiter, notifier) = thread_parker();
    let (ready_sender, ready_receiver) = mpsc::sync_channel(0);
    let join = thread::spawn(move || {
        let _ignored = ready_sender.send(());
        let mut duty = ();
        match waiter.wait(&mut duty, Span::from_nanos(5_000_000_000)) {
            Ok(outcome) => outcome,
            Err(never) => match never {},
        }
    });

    ready_receiver.recv()?;
    notifier.notify();
    let outcome = match join.join() {
        Ok(outcome) => outcome,
        Err(_) => panic!("waiter thread panicked"),
    };

    assert_eq!(outcome, WaitOutcome::Notified);
    Ok(())
}
