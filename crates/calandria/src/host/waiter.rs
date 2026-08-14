//! Duty-aware bounded waiting for dedicated owner threads.

use std::{
    cell::Cell,
    convert::Infallible,
    fmt, io,
    marker::PhantomData,
    sync::{Arc, Condvar, Mutex, MutexGuard},
};

use crate::{Span, WakeHandle, WakeSource};

/// Result of one bounded wait attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitOutcome {
    /// The waiter observed an external notification or readiness event.
    Notified,
    /// The waiter returned without an external notification.
    ///
    /// This includes both an elapsed maximum wait and a permitted spurious
    /// return. The host executes another bounded duty turn either way.
    Idle,
}

/// Bounded waiting strategy used by an owned reactor.
///
/// The waiter receives exclusive access to the owned duty. A simple thread
/// parker may ignore it, while an I/O waiter may poll readiness directly into
/// reactor-owned storage. This avoids shared mutable handoff between the
/// waiting backend and the single owner.
pub trait Waiter<D> {
    /// Failure returned by the waiting backend.
    type Error;

    /// Requests progress observation with `maximum` as the blocking bound.
    ///
    /// Implementations may return early or observe a spurious wake. They must
    /// not intentionally extend the backend wait beyond `maximum`; operating-
    /// system clock granularity and scheduling may still delay the return. Any
    /// work performed before or after the wait must itself be bounded, and
    /// implementations must not call
    /// [`Duty::turn`](super::Duty::turn). The host always executes another
    /// bounded duty turn after this method succeeds.
    fn wait(&mut self, duty: &mut D, maximum: Span) -> Result<WaitOutcome, Self::Error>;
}

impl<D, F, E> Waiter<D> for F
where
    F: FnMut(&mut D, Span) -> Result<WaitOutcome, E>,
{
    type Error = E;

    fn wait(&mut self, duty: &mut D, maximum: Span) -> Result<WaitOutcome, Self::Error> {
        self(duty, maximum)
    }
}

/// Single-owner condition-variable parker.
pub struct ThreadParker {
    shared: Arc<ThreadParkState>,
    _single_owner: PhantomData<Cell<()>>,
}

/// Cloneable producer side of a [`ThreadParker`].
///
/// Each independently acknowledged publication domain should call
/// [`Self::wake_handle`] rather than cloning one [`WakeHandle`] across domains.
#[derive(Clone)]
pub struct ThreadNotifier {
    shared: Arc<ThreadParkState>,
}

/// Creates one dedicated parker and its cloneable notification source.
pub fn thread_parker() -> (ThreadParker, ThreadNotifier) {
    let shared = Arc::new(ThreadParkState {
        notified: Mutex::new(false),
        changed: Condvar::new(),
    });
    (
        ThreadParker {
            shared: Arc::clone(&shared),
            _single_owner: PhantomData,
        },
        ThreadNotifier { shared },
    )
}

impl ThreadNotifier {
    /// Creates an independent coalesced wake domain targeting this parker.
    pub fn wake_handle(&self) -> WakeHandle {
        WakeHandle::new(self.clone())
    }

    /// Publishes a persistent coalesced notification token.
    pub fn notify(&self) {
        let mut notified = self.shared.lock();
        *notified = true;
        self.shared.changed.notify_one();
    }
}

impl WakeSource for ThreadNotifier {
    fn wake(&self) -> io::Result<()> {
        self.notify();
        Ok(())
    }
}

impl<D> Waiter<D> for ThreadParker {
    type Error = Infallible;

    fn wait(&mut self, _duty: &mut D, maximum: Span) -> Result<WaitOutcome, Self::Error> {
        let mut notified = self.shared.lock();
        if *notified {
            *notified = false;
            return Ok(WaitOutcome::Notified);
        }
        if maximum.as_nanos() == 0 {
            return Ok(WaitOutcome::Idle);
        }

        let waited =
            self.shared
                .changed
                .wait_timeout_while(notified, maximum.as_duration(), |pending| !*pending);
        let (mut notified, _timeout) = waited.unwrap_or_else(std::sync::PoisonError::into_inner);
        if *notified {
            *notified = false;
            Ok(WaitOutcome::Notified)
        } else {
            Ok(WaitOutcome::Idle)
        }
    }
}

impl fmt::Debug for ThreadParker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThreadParker")
            .field("notified", &*self.shared.lock())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ThreadNotifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThreadNotifier")
            .field("notified", &*self.shared.lock())
            .finish_non_exhaustive()
    }
}

struct ThreadParkState {
    notified: Mutex<bool>,
    changed: Condvar,
}

impl ThreadParkState {
    fn lock(&self) -> MutexGuard<'_, bool> {
        self.notified
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
