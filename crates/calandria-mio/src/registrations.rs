//! Bounded translation between wide resource tokens and backend tokens.

use std::vec::Vec;

use calandria::ResourceToken;
use mio::Token;

use crate::{MioError, MioPollerLimits, MioPollerSnapshot};

pub(super) const WAKE_TOKEN: Token = Token(0);

#[derive(Debug)]
pub(super) struct Registrations {
    limits: MioPollerLimits,
    entries: Vec<Registration>,
    next_backend: Option<usize>,
}

impl Registrations {
    pub(super) fn new(limits: MioPollerLimits) -> Self {
        Self {
            limits,
            entries: Vec::with_capacity(limits.registrations().get()),
            next_backend: Some(1),
        }
    }

    pub(super) const fn limits(&self) -> MioPollerLimits {
        self.limits
    }

    pub(super) fn reserve(&mut self, resource: ResourceToken) -> Result<Token, MioError> {
        if self.contains(resource) {
            return Err(MioError::AlreadyRegistered { token: resource });
        }
        if self.entries.len() >= self.limits.registrations().get() {
            return Err(MioError::RegistrationCapacity {
                limit: self.limits.registrations(),
            });
        }
        let Some(raw) = self.next_backend else {
            return Err(MioError::TokenSpaceExhausted);
        };
        self.next_backend = raw.checked_add(1);
        Ok(Token(raw))
    }

    pub(super) fn commit(&mut self, resource: ResourceToken, backend: Token) {
        assert!(
            !self.contains(resource) && self.resource(backend).is_none(),
            "Mio registration identity invariant violated"
        );
        assert!(
            self.entries.len() < self.limits.registrations().get(),
            "Mio registration capacity invariant violated"
        );
        if self
            .entries
            .last()
            .is_some_and(|entry| entry.backend.0 >= backend.0)
        {
            panic!("Mio backend token ordering invariant violated");
        }
        self.entries.push(Registration { resource, backend });
    }

    pub(super) fn backend(&self, resource: ResourceToken) -> Result<Token, MioError> {
        self.entries
            .iter()
            .find(|entry| entry.resource == resource)
            .map(|entry| entry.backend)
            .ok_or(MioError::NotRegistered { token: resource })
    }

    pub(super) fn resource(&self, backend: Token) -> Option<ResourceToken> {
        self.entries
            .binary_search_by_key(&backend.0, |entry| entry.backend.0)
            .ok()
            .map(|index| self.entries[index].resource)
    }

    pub(super) fn remove(&mut self, resource: ResourceToken) -> Result<(), MioError> {
        let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.resource == resource)
        else {
            return Err(MioError::NotRegistered { token: resource });
        };
        self.entries.remove(index);
        Ok(())
    }

    pub(super) fn snapshot(&self) -> MioPollerSnapshot {
        MioPollerSnapshot::new(self.limits, self.entries.len(), self.next_backend)
    }

    fn contains(&self, resource: ResourceToken) -> bool {
        self.entries.iter().any(|entry| entry.resource == resource)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Registration {
    resource: ResourceToken,
    backend: Token,
}
