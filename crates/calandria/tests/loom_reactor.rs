//! Loom models for reactor termination, group admission, and startup decisions.

use loom::{
    sync::{Arc, Condvar, Mutex},
    thread,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReactorPhase {
    Running,
    TerminationRequested,
    Exited,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestStatus {
    Requested,
    Exited,
}

#[test]
fn termination_publication_linearizes_before_a_failed_wake() {
    loom::model(|| {
        let phase = Arc::new(Mutex::new(ReactorPhase::Running));
        let owner_observed = Arc::new(Mutex::new(false));
        let request_phase = Arc::clone(&phase);
        let requester = thread::spawn(move || {
            let mut phase = request_phase
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let status = match *phase {
                ReactorPhase::Running => {
                    *phase = ReactorPhase::TerminationRequested;
                    RequestStatus::Requested
                }
                ReactorPhase::TerminationRequested => RequestStatus::Requested,
                ReactorPhase::Exited => RequestStatus::Exited,
            };
            drop(phase);
            thread::yield_now();
            status
        });
        let owner_phase = Arc::clone(&phase);
        let observed = Arc::clone(&owner_observed);
        let owner = thread::spawn(move || {
            let mut phase = owner_phase
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *observed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                *phase == ReactorPhase::TerminationRequested;
            *phase = ReactorPhase::Exited;
        });

        let request = requester
            .join()
            .unwrap_or_else(|_| panic!("modeled requester panicked"));
        owner
            .join()
            .unwrap_or_else(|_| panic!("modeled owner panicked"));
        assert_eq!(
            *phase
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ReactorPhase::Exited
        );
        if request == RequestStatus::Requested {
            assert!(
                *owner_observed
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
            );
        }
    });
}

#[derive(Debug)]
struct Admission {
    open: bool,
    accepted: usize,
}

#[test]
fn group_close_and_ingress_admission_share_one_linearization_lock() {
    loom::model(|| {
        let admission = Arc::new(Mutex::new(Admission {
            open: true,
            accepted: 0,
        }));
        let sender_state = Arc::clone(&admission);
        let sender = thread::spawn(move || {
            let mut state = sender_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !state.open {
                return false;
            }
            thread::yield_now();
            state.accepted += 1;
            true
        });
        let closer_state = Arc::clone(&admission);
        let closer = thread::spawn(move || {
            closer_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .open = false;
        });

        let accepted = sender
            .join()
            .unwrap_or_else(|_| panic!("modeled sender panicked"));
        closer
            .join()
            .unwrap_or_else(|_| panic!("modeled closer panicked"));
        let state = admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(!state.open);
        assert_eq!(state.accepted, usize::from(accepted));
    });
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StartDecision {
    Start,
    Abort,
}

#[test]
fn reactor_group_start_decision_is_one_shot() {
    loom::model(|| {
        let state = Arc::new((Mutex::new(None), Condvar::new()));
        let worker_state = Arc::clone(&state);
        let worker = thread::spawn(move || {
            let (decision, decided) = &*worker_state;
            let mut decision = decision
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            while decision.is_none() {
                decision = decided
                    .wait(decision)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            decision.unwrap_or_else(|| panic!("modeled start decision disappeared"))
        });
        let first_state = Arc::clone(&state);
        let first = thread::spawn(move || decide(&first_state, StartDecision::Start));
        let second_state = Arc::clone(&state);
        let second = thread::spawn(move || decide(&second_state, StartDecision::Abort));

        first
            .join()
            .unwrap_or_else(|_| panic!("modeled first decider panicked"));
        second
            .join()
            .unwrap_or_else(|_| panic!("modeled second decider panicked"));
        let observed = worker
            .join()
            .unwrap_or_else(|_| panic!("modeled worker panicked"));
        let final_decision = state
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .unwrap_or_else(|| panic!("modeled final decision missing"));
        assert_eq!(observed, final_decision);
    });
}

fn decide(state: &Arc<(Mutex<Option<StartDecision>>, Condvar)>, selected: StartDecision) {
    let (decision, decided) = &**state;
    let mut decision = decision
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if decision.is_none() {
        *decision = Some(selected);
        decided.notify_all();
    }
}
