//! Constructed single-owner reactor before local or dedicated execution.

use crate::{Clock, Duty, EmbeddedHost, HostConfig, Waiter, WakeHandle};

use super::{
    ReactorControl, ReactorExit, ReactorHandle, ReactorSpawnError,
    control::{ReactorControlOwner, control_pair},
    handle, runner,
};

/// One owned duty, clock, waiter, and bounded reactor run loop.
#[derive(Debug)]
pub struct Reactor<D, C, W> {
    host: EmbeddedHost<D, C>,
    waiter: W,
    control: ReactorControlOwner,
}

impl<D, C, W> Reactor<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    /// Creates a reactor using the default bounded host policy.
    pub fn new(duty: D, clock: C, waiter: W, termination_wake: WakeHandle) -> Self {
        Self::with_config(duty, clock, waiter, termination_wake, HostConfig::default())
    }

    /// Creates a reactor using an explicit bounded host policy.
    pub fn with_config(
        duty: D,
        clock: C,
        waiter: W,
        termination_wake: WakeHandle,
        config: HostConfig,
    ) -> Self {
        Self::from_host(
            EmbeddedHost::new(duty, clock, config),
            waiter,
            termination_wake,
        )
    }

    /// Creates a reactor from an already configured embedded host.
    pub fn from_host(host: EmbeddedHost<D, C>, waiter: W, termination_wake: WakeHandle) -> Self {
        let (control, _external) = control_pair(termination_wake);
        Self {
            host,
            waiter,
            control,
        }
    }

    /// Runs this reactor on the calling thread until terminal exit.
    pub fn run(self) -> ReactorExit<D, C, W> {
        runner::run(self.host, self.waiter, &self.control)
    }

    /// Starts this reactor on one named thread without losing ownership on failure.
    pub fn spawn(
        self,
        name: impl Into<String>,
    ) -> Result<ReactorHandle<D, C, W>, ReactorSpawnError<D, C, W>>
    where
        D: Send + 'static,
        D::Error: Send + 'static,
        C: Send + 'static,
        C::Error: Send + 'static,
        W: Send + 'static,
        W::Error: Send + 'static,
    {
        handle::spawn(name.into(), self)
    }

    /// Returns shared access to the owned duty.
    pub const fn duty(&self) -> &D {
        self.host.duty()
    }

    /// Returns exclusive access before execution starts.
    pub fn duty_mut(&mut self) -> &mut D {
        self.host.duty_mut()
    }

    /// Returns shared access to the waiting backend.
    pub const fn waiter(&self) -> &W {
        &self.waiter
    }

    /// Returns exclusive access to the waiting backend before execution starts.
    pub fn waiter_mut(&mut self) -> &mut W {
        &mut self.waiter
    }

    /// Consumes the reactor and returns its host and waiter without running.
    pub fn into_parts(self) -> (EmbeddedHost<D, C>, W) {
        (self.host, self.waiter)
    }

    pub(crate) fn control(&self) -> ReactorControl {
        self.control.control()
    }
}
