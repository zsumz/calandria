//! Shared phase and bounded subscriber ownership for a shutdown barrier.

use std::{mem, num::NonZeroUsize};

use crate::{
    Completer,
    sync::{Condvar, Mutex, MutexGuard, recover_poison},
};

pub(super) struct Shared {
    capacity: usize,
    state: Mutex<State>,
    changed: Condvar,
}

pub(super) struct State {
    pub(super) phase: Phase,
    pub(super) subscribers: Vec<Completer<()>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Open,
    Requesting,
    Requested,
    Completed,
    Closed,
}

pub(super) enum RequestSuccess {
    Admitted,
    Completed(Completer<()>),
    Closed(Completer<()>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestFailure {
    Reopened,
    Completed,
    Closed,
}

impl Shared {
    pub(super) fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity: capacity.get(),
            state: Mutex::new(State {
                phase: Phase::Open,
                subscribers: Vec::with_capacity(capacity.get()),
            }),
            changed: Condvar::new(),
        }
    }

    pub(super) const fn capacity(&self) -> usize {
        self.capacity
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(recover_poison)
    }

    pub(super) fn wait<'a>(&self, state: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
        self.changed.wait(state).unwrap_or_else(recover_poison)
    }

    pub(super) fn finish_request_success(&self, completer: Completer<()>) -> RequestSuccess {
        let mut state = self.lock();
        let result = match state.phase {
            Phase::Requesting => {
                state.subscribers.push(completer);
                state.phase = Phase::Requested;
                RequestSuccess::Admitted
            }
            Phase::Completed => RequestSuccess::Completed(completer),
            Phase::Closed => RequestSuccess::Closed(completer),
            Phase::Open | Phase::Requested => panic!("shutdown request-success invariant violated"),
        };
        drop(state);
        self.changed.notify_all();
        result
    }

    pub(super) fn finish_request_failure(&self) -> RequestFailure {
        let mut state = self.lock();
        let result = match state.phase {
            Phase::Requesting => {
                state.phase = Phase::Open;
                RequestFailure::Reopened
            }
            Phase::Completed => RequestFailure::Completed,
            Phase::Closed => RequestFailure::Closed,
            Phase::Open | Phase::Requested => panic!("shutdown request-failure invariant violated"),
        };
        drop(state);
        self.changed.notify_all();
        result
    }

    pub(super) fn abort_request(&self) {
        let mut state = self.lock();
        if state.phase == Phase::Requesting {
            state.phase = Phase::Open;
            drop(state);
            self.changed.notify_all();
        }
    }

    pub(super) fn settle(&self, terminal: Phase) -> Vec<Completer<()>> {
        debug_assert!(matches!(terminal, Phase::Completed | Phase::Closed));
        let mut state = self.lock();
        if matches!(state.phase, Phase::Completed | Phase::Closed) {
            return Vec::new();
        }
        state.phase = terminal;
        let subscribers = mem::take(&mut state.subscribers);
        drop(state);
        self.changed.notify_all();
        subscribers
    }
}

pub(super) struct RequestAttempt<'a> {
    shared: &'a Shared,
    armed: bool,
}

impl<'a> RequestAttempt<'a> {
    pub(super) const fn new(shared: &'a Shared) -> Self {
        Self {
            shared,
            armed: true,
        }
    }

    pub(super) const fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RequestAttempt<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.shared.abort_request();
        }
    }
}
