//! Linearized bounded typed ingress for a static reactor topology.

use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex, MutexGuard},
};

use crate::{Lane, MailboxSender, MailboxSnapshot, RetainedBytes};

use super::{ReactorGroupSendError, ReactorGroupSendFailure, ReactorId};

/// Cloneable bounded ingress handle for a static reactor group.
pub struct ReactorGroupHandle<T> {
    shared: Arc<Shared<T>>,
}

pub(super) struct AdmissionCloser<T> {
    shared: Arc<Shared<T>>,
}

pub(super) fn bounded_ingress<T>(
    senders: Vec<MailboxSender<T>>,
) -> (ReactorGroupHandle<T>, AdmissionCloser<T>) {
    let reactors = NonZeroUsize::new(senders.len())
        .unwrap_or_else(|| panic!("validated reactor group topology became empty"));
    let shared = Arc::new(Shared {
        reactors,
        state: Mutex::new(State {
            open: true,
            senders: senders.into_boxed_slice(),
        }),
    });
    (
        ReactorGroupHandle {
            shared: Arc::clone(&shared),
        },
        AdmissionCloser { shared },
    )
}

impl<T> ReactorGroupHandle<T> {
    /// Attempts to route ordinary work to one exact reactor.
    pub fn try_send(&self, reactor: ReactorId, item: T) -> Result<(), ReactorGroupSendError<T>> {
        self.try_send_to(reactor, Lane::Work, item)
    }

    /// Attempts to route control work to one exact reactor.
    pub fn try_send_control(
        &self,
        reactor: ReactorId,
        item: T,
    ) -> Result<(), ReactorGroupSendError<T>> {
        self.try_send_to(reactor, Lane::Control, item)
    }

    /// Attempts to route work through a selected bounded mailbox lane.
    pub fn try_send_to(
        &self,
        reactor: ReactorId,
        lane: Lane,
        item: T,
    ) -> Result<(), ReactorGroupSendError<T>> {
        let state = self.shared.lock();
        if !state.open {
            return Err(ReactorGroupSendError::new(
                item,
                lane,
                ReactorGroupSendFailure::Closed,
            ));
        }
        let Some(sender) = reactor
            .position()
            .and_then(|position| state.senders.get(position))
        else {
            return Err(ReactorGroupSendError::new(
                item,
                lane,
                ReactorGroupSendFailure::UnknownReactor {
                    reactor,
                    reactors: self.shared.reactors,
                },
            ));
        };
        sender.try_send_to(lane, item).map_err(|error| {
            let (item, rejected_lane, failure) = error.into_parts();
            ReactorGroupSendError::new(
                item,
                rejected_lane,
                ReactorGroupSendFailure::Mailbox(failure),
            )
        })
    }

    /// Proves group and mailbox admission before materializing a queued value.
    ///
    /// Both callbacks run while group admission is exclusively locked;
    /// `materialize` also runs under the selected mailbox lock. They must be
    /// fast, deterministic, and must not call back into this group handle.
    pub fn try_send_materialized<U>(
        &self,
        reactor: ReactorId,
        lane: Lane,
        owner: U,
        retained_bytes: impl FnOnce(&U) -> RetainedBytes,
        materialize: impl FnOnce(U) -> T,
    ) -> Result<(), ReactorGroupSendError<U>> {
        let state = self.shared.lock();
        if !state.open {
            return Err(ReactorGroupSendError::new(
                owner,
                lane,
                ReactorGroupSendFailure::Closed,
            ));
        }
        let Some(sender) = reactor
            .position()
            .and_then(|position| state.senders.get(position))
        else {
            return Err(ReactorGroupSendError::new(
                owner,
                lane,
                ReactorGroupSendFailure::UnknownReactor {
                    reactor,
                    reactors: self.shared.reactors,
                },
            ));
        };
        sender
            .try_send_materialized(lane, owner, retained_bytes, materialize)
            .map_err(|error| {
                let (item, rejected_lane, failure) = error.into_parts();
                ReactorGroupSendError::new(
                    item,
                    rejected_lane,
                    ReactorGroupSendFailure::Mailbox(failure),
                )
            })
    }

    /// Returns the immutable reactor count.
    pub fn reactors(&self) -> NonZeroUsize {
        self.shared.reactors
    }

    /// Returns whether group admission currently accepts routing attempts.
    pub fn is_open(&self) -> bool {
        self.shared.lock().open
    }

    /// Returns one reactor mailbox snapshot when the identity is valid.
    pub fn mailbox_snapshot(&self, reactor: ReactorId) -> Option<MailboxSnapshot> {
        let state = self.shared.lock();
        reactor
            .position()
            .and_then(|position| state.senders.get(position))
            .map(MailboxSender::snapshot)
    }

    pub(super) fn into_senders(self) -> Vec<MailboxSender<T>> {
        let shared = Arc::try_unwrap(self.shared)
            .unwrap_or_else(|_| panic!("reactor group ingress ownership invariant violated"));
        shared
            .state
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .senders
            .into_vec()
    }
}

impl<T> Clone for ReactorGroupHandle<T> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<T> AdmissionCloser<T> {
    pub(super) fn close(&self) {
        self.shared.lock().open = false;
    }
}

impl<T> Clone for AdmissionCloser<T> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<T> core::fmt::Debug for ReactorGroupHandle<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ReactorGroupHandle")
            .field("reactors", &self.shared.reactors)
            .field("open", &self.is_open())
            .finish_non_exhaustive()
    }
}

struct Shared<T> {
    reactors: NonZeroUsize,
    state: Mutex<State<T>>,
}

impl<T> Shared<T> {
    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

struct State<T> {
    open: bool,
    senders: Box<[MailboxSender<T>]>,
}
