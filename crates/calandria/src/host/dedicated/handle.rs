//! Join ownership for a dedicated host thread.

use std::{
    io,
    thread::{self, JoinHandle, Thread},
};

use super::{DedicatedExit, runner::run};
use crate::host::{Clock, Duty, EmbeddedHost, Waiter};

/// Join handle for one thread-owned bounded host.
#[must_use = "dropping the handle detaches lifecycle observation and does not stop the duty"]
#[derive(Debug)]
pub struct DedicatedHost<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    join: JoinHandle<DedicatedExit<D, C, W>>,
}

impl<D, C, W> DedicatedHost<D, C, W>
where
    D: Duty + Send + 'static,
    D::Error: Send + 'static,
    C: Clock + Send + 'static,
    C::Error: Send + 'static,
    W: Waiter<D> + Send + 'static,
    W::Error: Send + 'static,
{
    /// Starts an owned thread that turns, waits, and terminates the host.
    pub fn spawn(
        name: impl Into<String>,
        host: EmbeddedHost<D, C>,
        waiter: W,
    ) -> io::Result<Self> {
        let join = thread::Builder::new()
            .name(name.into())
            .spawn(move || run(host, waiter))?;
        Ok(Self { join })
    }

    /// Returns whether the dedicated thread has terminated.
    pub fn is_finished(&self) -> bool {
        self.join.is_finished()
    }

    /// Returns the underlying thread handle.
    pub fn thread(&self) -> &Thread {
        self.join.thread()
    }

    /// Joins the dedicated thread.
    ///
    /// A panic is returned through the standard thread join result. Calandria
    /// does not silently translate or discard the panic payload.
    pub fn join(self) -> thread::Result<DedicatedExit<D, C, W>> {
        self.join.join()
    }
}
