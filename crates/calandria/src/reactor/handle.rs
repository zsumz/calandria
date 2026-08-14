//! Join and fail-safe termination ownership for one reactor thread.

use std::{
    io,
    sync::{Arc, Mutex, MutexGuard},
    thread::{self, JoinHandle, Thread},
};

use crate::{Clock, Duty, Waiter};

use super::{Reactor, ReactorControl, ReactorExit, ReactorSpawnError, ReactorTermination};

/// Join handle for one thread-owned reactor.
#[must_use = "dropping the handle detaches observation and does not terminate the reactor"]
#[derive(Debug)]
pub struct ReactorHandle<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    join: JoinHandle<ReactorExit<D, C, W>>,
    control: ReactorControl,
}

impl<D, C, W> ReactorHandle<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    /// Requests fail-safe termination after the current bounded operation.
    pub fn request_termination(&self) -> ReactorTermination {
        self.control.request_termination()
    }

    /// Returns whether the reactor thread has terminated.
    pub fn is_finished(&self) -> bool {
        self.join.is_finished()
    }

    /// Returns the underlying thread handle.
    pub fn thread(&self) -> &Thread {
        self.join.thread()
    }

    /// Joins the reactor thread, preserving a panic as a standard join error.
    pub fn join(self) -> thread::Result<ReactorExit<D, C, W>> {
        self.join.join()
    }
}

pub(super) fn spawn<D, C, W>(
    name: String,
    reactor: Reactor<D, C, W>,
) -> Result<ReactorHandle<D, C, W>, ReactorSpawnError<D, C, W>>
where
    D: Duty + Send + 'static,
    D::Error: Send + 'static,
    C: Clock + Send + 'static,
    C::Error: Send + 'static,
    W: Waiter<D> + Send + 'static,
    W::Error: Send + 'static,
{
    if name.as_bytes().contains(&0) {
        return Err(ReactorSpawnError::new(invalid_thread_name(), reactor));
    }
    let control = reactor.control();
    let slot = Arc::new(Mutex::new(Some(reactor)));
    let thread_slot = Arc::clone(&slot);
    let spawned = thread::Builder::new().name(name).spawn(move || {
        let reactor = take(&thread_slot);
        reactor.run()
    });
    match spawned {
        Ok(join) => Ok(ReactorHandle { join, control }),
        Err(source) => Err(ReactorSpawnError::new(source, take(&slot))),
    }
}

pub(crate) fn invalid_thread_name() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "thread name contains an interior null byte",
    )
}

fn take<D, C, W>(slot: &Mutex<Option<Reactor<D, C, W>>>) -> Reactor<D, C, W> {
    lock(slot)
        .take()
        .unwrap_or_else(|| panic!("reactor spawn ownership invariant violated"))
}

fn lock<T>(slot: &Mutex<T>) -> MutexGuard<'_, T> {
    slot.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
