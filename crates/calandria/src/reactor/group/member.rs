//! Atomic construction of an unstarted reactor and its bounded ingress authority.

use crate::{
    MailboxLimits, MailboxReceiver, MailboxSender, Reactor, Retained, RetainedBytes, WakeHandle,
    mailbox, mailbox_with,
};

/// One unstarted reactor and the exact mailbox sender routed to its duty.
#[derive(Debug)]
pub struct ReactorGroupMember<D, C, W, T> {
    reactor: Reactor<D, C, W>,
    ingress: MailboxSender<T>,
}

impl<D, C, W, T> ReactorGroupMember<D, C, W, T> {
    /// Mints a bounded mailbox and constructs its sole reactor owner.
    ///
    /// The sender is not exposed before group admission wraps it. The factory
    /// must move the supplied receiver into the returned reactor duty.
    pub fn with_mailbox(
        limits: MailboxLimits,
        wake: WakeHandle,
        reactor: impl FnOnce(MailboxReceiver<T>) -> Reactor<D, C, W>,
    ) -> Self
    where
        T: Retained,
    {
        let (ingress, receiver) = mailbox(limits, wake);
        Self {
            reactor: reactor(receiver),
            ingress,
        }
    }

    /// Mints a measured bounded mailbox and constructs its sole reactor owner.
    ///
    /// The measurement function follows [`crate::mailbox_with`]. The factory
    /// must move the supplied receiver into the returned reactor duty.
    pub fn with_mailbox_measure(
        limits: MailboxLimits,
        measure: fn(&T) -> RetainedBytes,
        wake: WakeHandle,
        reactor: impl FnOnce(MailboxReceiver<T>) -> Reactor<D, C, W>,
    ) -> Self {
        let (ingress, receiver) = mailbox_with(limits, measure, wake);
        Self {
            reactor: reactor(receiver),
            ingress,
        }
    }

    pub(super) const fn from_parts(reactor: Reactor<D, C, W>, ingress: MailboxSender<T>) -> Self {
        Self { reactor, ingress }
    }

    /// Returns shared access to the unstarted reactor.
    pub const fn reactor(&self) -> &Reactor<D, C, W> {
        &self.reactor
    }

    pub(super) fn into_parts(self) -> (Reactor<D, C, W>, MailboxSender<T>) {
        (self.reactor, self.ingress)
    }
}
