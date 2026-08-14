//! Mutex and condition-variable state for one completion pair.

use std::{
    mem,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    task::{Context, Poll, Waker},
};

use super::CompletionError;

pub(super) struct Shared<T> {
    inner: Arc<Inner<T>>,
}

struct Inner<T> {
    state: Mutex<State<T>>,
    ready: Condvar,
}

enum State<T> {
    Pending {
        observer_alive: bool,
        waker: Option<Waker>,
    },
    Ready(Result<T, CompletionError>),
    Consumed,
}

impl<T> Shared<T> {
    pub(super) fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State::Pending {
                    observer_alive: true,
                    waker: None,
                }),
                ready: Condvar::new(),
            }),
        }
    }

    pub(super) fn complete(&self, value: T) -> Result<(), T> {
        let mut state = self.lock();
        let previous = mem::replace(&mut *state, State::Consumed);
        let (result, waker) = match previous {
            State::Pending {
                observer_alive: true,
                waker,
            } => {
                *state = State::Ready(Ok(value));
                (Ok(()), waker)
            }
            State::Pending {
                observer_alive: false,
                ..
            }
            | State::Consumed => (Err(value), None),
            ready @ State::Ready(_) => {
                *state = ready;
                (Err(value), None)
            }
        };
        drop(state);
        self.inner.ready.notify_all();
        wake(waker);
        result
    }

    pub(super) fn close_producer(&self) {
        let mut state = self.lock();
        let previous = mem::replace(&mut *state, State::Consumed);
        let waker = match previous {
            State::Pending {
                observer_alive: true,
                waker,
            } => {
                *state = State::Ready(Err(CompletionError::Closed));
                waker
            }
            State::Pending { .. } | State::Consumed => None,
            ready @ State::Ready(_) => {
                *state = ready;
                None
            }
        };
        drop(state);
        self.inner.ready.notify_all();
        wake(waker);
    }

    pub(super) fn wait(&self) -> Result<T, CompletionError> {
        let mut state = self.lock();
        loop {
            if matches!(&*state, State::Pending { .. }) {
                state = self.wait_until_ready(state);
                continue;
            }
            return match mem::replace(&mut *state, State::Consumed) {
                State::Ready(result) => result,
                State::Consumed => Err(CompletionError::Consumed),
                State::Pending { .. } => panic!("completion wait invariant violated"),
            };
        }
    }

    pub(super) fn try_take(&self) -> Option<Result<T, CompletionError>> {
        let mut state = self.lock();
        if matches!(&*state, State::Pending { .. }) {
            return None;
        }
        Some(match mem::replace(&mut *state, State::Consumed) {
            State::Ready(result) => result,
            State::Consumed => Err(CompletionError::Consumed),
            State::Pending { .. } => panic!("completion extraction invariant violated"),
        })
    }

    pub(super) fn poll(&self, context: &Context<'_>) -> Poll<Result<T, CompletionError>> {
        let mut state = self.lock();
        if let State::Pending { waker, .. } = &mut *state {
            if !waker
                .as_ref()
                .is_some_and(|stored| stored.will_wake(context.waker()))
            {
                *waker = Some(context.waker().clone());
            }
            return Poll::Pending;
        }
        match mem::replace(&mut *state, State::Consumed) {
            State::Ready(result) => Poll::Ready(result),
            State::Consumed => Poll::Ready(Err(CompletionError::Consumed)),
            State::Pending { .. } => panic!("completion poll invariant violated"),
        }
    }

    pub(super) fn abandon_observer(&self) {
        let mut state = self.lock();
        let previous = mem::replace(&mut *state, State::Consumed);
        let discarded = match previous {
            State::Pending { .. } => {
                *state = State::Pending {
                    observer_alive: false,
                    waker: None,
                };
                None
            }
            State::Ready(result) => Some(result),
            State::Consumed => None,
        };
        drop(state);
        drop(discarded);
    }

    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn wait_until_ready<'a>(&self, state: MutexGuard<'a, State<T>>) -> MutexGuard<'a, State<T>> {
        self.inner
            .ready
            .wait(state)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

fn wake(waker: Option<Waker>) {
    if let Some(waker) = waker {
        waker.wake();
    }
}
