//! Loom schedules over production reactor termination publication.

use std::io;

use loom::thread;

use crate::{ReactorTerminationStatus, WakeHandle};

use super::control::control_pair;

#[test]
fn termination_publication_precedes_owner_exit_and_failed_wake() {
    loom::model(|| {
        let wake = WakeHandle::new(|| {
            thread::yield_now();
            Err(io::Error::other("modeled wake failure"))
        });
        let (owner, control) = control_pair(wake);
        let requester = thread::spawn(move || control.request_termination());
        let exiting = thread::spawn(move || owner.finish());

        let request = requester
            .join()
            .unwrap_or_else(|_| panic!("production termination requester panicked"));
        let observed = exiting
            .join()
            .unwrap_or_else(|_| panic!("production reactor owner panicked"));

        match request.status() {
            ReactorTerminationStatus::Requested => {
                assert!(observed);
                assert!(request.wake_error().is_some());
            }
            ReactorTerminationStatus::Exited => {
                assert!(!observed);
                assert!(request.wake_error().is_none());
            }
            ReactorTerminationStatus::AlreadyRequested => {
                panic!("one request cannot observe an earlier request")
            }
        }
    });
}
