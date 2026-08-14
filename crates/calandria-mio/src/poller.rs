//! Mio selector ownership with bounded event and registration translation.

use std::sync::Arc;

use calandria::{
    Interest, PollEvent, PollEvents, PollReport, Poller, ResourceToken, Span, WakeHandle,
};
use mio::{Events, Poll, Waker, event::Source};

use crate::{
    MioError, MioPollerLimits, MioPollerSnapshot,
    registrations::{Registrations, WAKE_TOKEN},
    translation::{into_mio_interest, readiness},
};

/// One owner-local Mio selector and its bounded translation state.
#[derive(Debug)]
pub struct MioPoller {
    poll: Poll,
    events: Events,
    waker: Arc<Waker>,
    registrations: Registrations,
}

impl MioPoller {
    /// Creates one poller and reserves backend token zero for wakes.
    pub fn new(limits: MioPollerLimits) -> Result<Self, MioError> {
        let poll = Poll::new()?;
        let waker = Arc::new(Waker::new(poll.registry(), WAKE_TOKEN)?);
        Ok(Self {
            poll,
            events: Events::with_capacity(limits.events().get()),
            waker,
            registrations: Registrations::new(limits),
        })
    }

    /// Allocates a reusable destination sized for one complete backend batch.
    pub fn event_batch(&self) -> PollEvents {
        PollEvents::new(self.limits().events())
    }

    /// Creates an independent coalesced wake domain for this selector.
    pub fn wake_handle(&self) -> WakeHandle {
        let waker = Arc::clone(&self.waker);
        WakeHandle::new(move || waker.wake())
    }

    /// Registers one exact resource generation.
    ///
    /// Backend identities are consumed monotonically even when Mio rejects
    /// registration. They are never reused for another resource generation.
    /// The concrete owner must retain this same source and use this poller for
    /// every later reregistration and deregistration.
    pub fn register<S: Source + ?Sized>(
        &mut self,
        source: &mut S,
        token: ResourceToken,
        interest: Interest,
    ) -> Result<(), MioError> {
        let backend_interest = into_mio_interest(interest)?;
        let backend = self.registrations.reserve(token)?;
        self.poll
            .registry()
            .register(source, backend, backend_interest)?;
        self.registrations.commit(token, backend);
        Ok(())
    }

    /// Changes readiness interest for one active exact resource generation.
    ///
    /// `source` must be the source originally registered with this poller.
    pub fn reregister<S: Source + ?Sized>(
        &mut self,
        source: &mut S,
        token: ResourceToken,
        interest: Interest,
    ) -> Result<(), MioError> {
        let backend = self.registrations.backend(token)?;
        let backend_interest = into_mio_interest(interest)?;
        self.poll
            .registry()
            .reregister(source, backend, backend_interest)?;
        Ok(())
    }

    /// Removes one exact resource generation from the selector.
    ///
    /// `source` must be the source originally registered with this poller. A
    /// concrete owner must deregister a source before dropping it unless the
    /// source type documents a different lifecycle.
    pub fn deregister<S: Source + ?Sized>(
        &mut self,
        source: &mut S,
        token: ResourceToken,
    ) -> Result<(), MioError> {
        self.registrations.backend(token)?;
        self.poll.registry().deregister(source)?;
        self.registrations.remove(token)
    }

    /// Performs one bounded poll through the core [`Poller`] contract.
    pub fn poll(
        &mut self,
        maximum: Span,
        destination: &mut PollEvents,
    ) -> Result<PollReport, MioError> {
        self.poll_once(maximum, destination)
    }

    /// Returns current bounded registration state.
    pub fn snapshot(&self) -> MioPollerSnapshot {
        self.registrations.snapshot()
    }

    /// Returns this poller's fixed event and registration limits.
    pub const fn limits(&self) -> MioPollerLimits {
        self.registrations.limits()
    }

    fn poll_once(
        &mut self,
        maximum: Span,
        destination: &mut PollEvents,
    ) -> Result<PollReport, MioError> {
        destination.clear();
        let required = self.limits().events();
        if destination.capacity().get() < required.get() {
            return Err(MioError::DestinationTooSmall {
                required,
                actual: destination.capacity(),
            });
        }

        self.poll
            .poll(&mut self.events, Some(maximum.as_duration()))?;
        let mut observed = 0;
        let mut wakes = 0;
        let mut stale = 0;

        for event in &self.events {
            observed += 1;
            let translated = if event.token() == WAKE_TOKEN {
                wakes += 1;
                Some(PollEvent::Wake)
            } else if let Some(token) = self.registrations.resource(event.token()) {
                Some(PollEvent::Resource {
                    token,
                    readiness: readiness(event),
                })
            } else {
                stale += 1;
                None
            };

            if let Some(translated) = translated {
                destination
                    .try_push(translated)
                    .unwrap_or_else(|_| panic!("validated poll destination capacity diverged"));
            }
        }

        Ok(PollReport::new(
            destination.len(),
            wakes,
            stale,
            observed >= required.get(),
        ))
    }
}

impl Poller for MioPoller {
    type Error = MioError;

    fn poll(
        &mut self,
        maximum: Span,
        destination: &mut PollEvents,
    ) -> Result<PollReport, Self::Error> {
        self.poll_once(maximum, destination)
    }
}
