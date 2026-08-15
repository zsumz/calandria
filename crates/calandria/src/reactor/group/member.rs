//! Atomic construction of an unstarted reactor and its bounded ingress authority.

use crate::{
    MailboxLimits, MailboxReceiver, MailboxSender, Reactor, Retained, RetainedBytes, WakeHandle,
    mailbox, mailbox_with,
};

use super::ReactorId;

/// One unstarted reactor and the exact mailbox sender routed to its duty.
#[derive(Debug)]
pub struct ReactorGroupMember<D, C, W, T> {
    id: ReactorId,
    reactor: Reactor<D, C, W>,
    ingress: MailboxSender<T>,
}

impl<D, C, W, T> ReactorGroupMember<D, C, W, T> {
    /// Mints a bounded mailbox and constructs its sole reactor owner.
    ///
    /// The declared identity is passed to the factory and must match this
    /// member's eventual position in the static topology.
    ///
    /// The sender is not exposed before group admission wraps it. The factory
    /// must move the supplied receiver into the returned reactor duty.
    pub fn with_mailbox(
        id: ReactorId,
        limits: MailboxLimits,
        wake: WakeHandle,
        reactor: impl FnOnce(ReactorId, MailboxReceiver<T>) -> Reactor<D, C, W>,
    ) -> Self
    where
        T: Retained,
    {
        let (ingress, receiver) = mailbox(limits, wake);
        Self {
            id,
            reactor: reactor(id, receiver),
            ingress,
        }
    }

    /// Mints a measured bounded mailbox and constructs its sole reactor owner.
    ///
    /// The declared identity is passed to the factory and must match this
    /// member's eventual position in the static topology.
    ///
    /// The measurement function follows [`crate::mailbox_with`]. The factory
    /// must move the supplied receiver into the returned reactor duty.
    pub fn with_mailbox_measure(
        id: ReactorId,
        limits: MailboxLimits,
        measure: fn(&T) -> RetainedBytes,
        wake: WakeHandle,
        reactor: impl FnOnce(ReactorId, MailboxReceiver<T>) -> Reactor<D, C, W>,
    ) -> Self {
        let (ingress, receiver) = mailbox_with(limits, measure, wake);
        Self {
            id,
            reactor: reactor(id, receiver),
            ingress,
        }
    }

    pub(super) const fn from_parts(
        id: ReactorId,
        reactor: Reactor<D, C, W>,
        ingress: MailboxSender<T>,
    ) -> Self {
        Self {
            id,
            reactor,
            ingress,
        }
    }

    /// Returns the declared static topology identity.
    pub const fn id(&self) -> ReactorId {
        self.id
    }

    /// Returns shared access to the unstarted reactor.
    pub const fn reactor(&self) -> &Reactor<D, C, W> {
        &self.reactor
    }

    pub(super) fn into_parts(self) -> (ReactorId, Reactor<D, C, W>, MailboxSender<T>) {
        (self.id, self.reactor, self.ingress)
    }
}
