//! One-shot start or abort gate for ownership-safe group startup.

use crate::sync::{Arc, Condvar, Mutex, MutexGuard, recover_poison};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StartDecision {
    Start,
    Abort,
}

#[derive(Clone)]
pub(super) struct StartGate {
    shared: Arc<Shared>,
}

impl StartGate {
    pub(super) fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                decision: Mutex::new(None),
                decided: Condvar::new(),
            }),
        }
    }

    pub(super) fn start(&self) {
        self.decide(StartDecision::Start);
    }

    pub(super) fn abort(&self) {
        self.decide(StartDecision::Abort);
    }

    pub(super) fn wait(&self) -> StartDecision {
        let mut decision = self.shared.lock();
        while decision.is_none() {
            decision = self
                .shared
                .decided
                .wait(decision)
                .unwrap_or_else(recover_poison);
        }
        decision.unwrap_or_else(|| panic!("reactor group start decision disappeared"))
    }

    fn decide(&self, selected: StartDecision) {
        let mut decision = self.shared.lock();
        if decision.is_none() {
            *decision = Some(selected);
            self.shared.decided.notify_all();
        }
    }
}

struct Shared {
    decision: Mutex<Option<StartDecision>>,
    decided: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Option<StartDecision>> {
        self.decision.lock().unwrap_or_else(recover_poison)
    }
}
