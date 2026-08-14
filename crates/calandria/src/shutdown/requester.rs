//! Cloneable authority for bounded shutdown observation and first request publication.

use std::{fmt, sync::Arc};

use crate::{Completer, Completion, completion};

use super::{
    ShutdownSubscribeError,
    shared::{Phase, RequestAttempt, RequestFailure, RequestSuccess, Shared},
};

/// Cloneable authority for registering bounded observers of one shutdown.
#[derive(Clone)]
pub struct ShutdownRequester {
    pub(super) shared: Arc<Shared>,
}

impl ShutdownRequester {
    /// Registers one observer and publishes the shutdown request once.
    ///
    /// The first caller that observes an open barrier runs `request`. The
    /// callback runs outside the barrier lock. Concurrent subscribers wait for
    /// that attempt; after success they join the same bounded terminal barrier.
    /// If the attempt fails, the barrier reopens and a later caller may retry.
    ///
    /// Capacity is proved before an observer is retained. A completed barrier
    /// accepts late observers without retaining them or calling `request`. An
    /// abandoned observer still occupies its retained registration until the
    /// barrier reaches a terminal phase.
    ///
    /// `request` must not synchronously subscribe to this same barrier: the
    /// nested subscription would wait for the request attempt currently running.
    pub fn subscribe<E>(
        &self,
        request: impl FnOnce() -> Result<(), E>,
    ) -> Result<Completion<()>, ShutdownSubscribeError<E>> {
        let (completion, completer) = completion();
        let mut completer = Some(completer);
        let mut request = Some(request);

        loop {
            let mut state = self.shared.lock();
            match state.phase {
                Phase::Open => {
                    state.phase = Phase::Requesting;
                    drop(state);
                    return self.publish_first_request(
                        take_request(&mut request),
                        take_completer(&mut completer),
                        completion,
                    );
                }
                Phase::Requesting => drop(self.shared.wait(state)),
                Phase::Requested => {
                    if state.subscribers.len() >= self.shared.capacity() {
                        return Err(ShutdownSubscribeError::Full);
                    }
                    state.subscribers.push(take_completer(&mut completer));
                    return Ok(completion);
                }
                Phase::Completed => {
                    drop(state);
                    let _ = take_completer(&mut completer).complete(());
                    return Ok(completion);
                }
                Phase::Closed => return Err(ShutdownSubscribeError::Closed),
            }
        }
    }

    fn publish_first_request<E>(
        &self,
        request: impl FnOnce() -> Result<(), E>,
        completer: Completer<()>,
        completion: Completion<()>,
    ) -> Result<Completion<()>, ShutdownSubscribeError<E>> {
        let mut attempt = RequestAttempt::new(&self.shared);
        let requested = request();
        attempt.disarm();
        match requested {
            Ok(()) => self.finish_success(completer, completion),
            Err(error) => self.finish_failure(error, completer, completion),
        }
    }

    fn finish_success<E>(
        &self,
        completer: Completer<()>,
        completion: Completion<()>,
    ) -> Result<Completion<()>, ShutdownSubscribeError<E>> {
        match self.shared.finish_request_success(completer) {
            RequestSuccess::Admitted => Ok(completion),
            RequestSuccess::Completed(completer) => {
                let _ = completer.complete(());
                Ok(completion)
            }
            RequestSuccess::Closed(completer) => {
                drop(completer);
                Err(ShutdownSubscribeError::Closed)
            }
        }
    }

    fn finish_failure<E>(
        &self,
        error: E,
        completer: Completer<()>,
        completion: Completion<()>,
    ) -> Result<Completion<()>, ShutdownSubscribeError<E>> {
        match self.shared.finish_request_failure() {
            RequestFailure::Reopened => Err(ShutdownSubscribeError::Request(error)),
            RequestFailure::Completed => {
                let _ = completer.complete(());
                Ok(completion)
            }
            RequestFailure::Closed => {
                drop(completer);
                Err(ShutdownSubscribeError::Closed)
            }
        }
    }
}

impl fmt::Debug for ShutdownRequester {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ShutdownRequester")
            .finish_non_exhaustive()
    }
}

fn take_completer(slot: &mut Option<Completer<()>>) -> Completer<()> {
    slot.take()
        .unwrap_or_else(|| panic!("shutdown completer ownership invariant violated"))
}

fn take_request<F>(slot: &mut Option<F>) -> F {
    slot.take()
        .unwrap_or_else(|| panic!("shutdown request ownership invariant violated"))
}
