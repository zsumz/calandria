//! Internal exact lookup and rollback support for deterministic actions.

use calandria::{Moment, Retained, RetainedBytes};

use crate::{EventId, EventToken};

use super::{Entry, Timeline};

impl<E: Retained> Timeline<E> {
    pub(crate) fn advance_to(&mut self, requested: Moment) {
        self.clock
            .advance_to(requested)
            .unwrap_or_else(|_| panic!("timeline cannot move backward"));
    }

    pub(crate) fn retained_for(&self, token: EventToken) -> Option<RetainedBytes> {
        if token.timeline() != self.id {
            return None;
        }
        self.events
            .get(&(token.at(), token.id()))
            .map(|entry| entry.retained)
    }

    pub(crate) fn event(&self, token: EventToken) -> Option<&E> {
        if token.timeline() != self.id {
            return None;
        }
        self.events
            .get(&(token.at(), token.id()))
            .map(|entry| &entry.event)
    }

    pub(crate) fn tokens_at(&self, at: Moment) -> impl Iterator<Item = EventToken> + '_ {
        self.events
            .range((at, EventId::MIN)..=(at, EventId::MAX))
            .map(move |((moment, id), _)| EventToken::new(self.id, *id, *moment))
    }

    pub(crate) fn pending(&self) -> impl Iterator<Item = (EventToken, &E)> + '_ {
        self.events.iter().map(move |((at, id), entry)| {
            (EventToken::new(self.id, *id, *at), &entry.event)
        })
    }

    pub(crate) fn restore(&mut self, token: EventToken, event: E) {
        assert_eq!(token.timeline(), self.id, "restored event must belong to timeline");
        assert!(token.at() >= self.now(), "restored event cannot be in the past");
        assert!(
            self.events.len() < self.limits.pending_events().get(),
            "restored event must fit its previously owned count"
        );

        let retained = event.retained_bytes();
        let next_retained = self
            .retained
            .checked_add(retained)
            .unwrap_or_else(|| panic!("restored event accounting overflowed"));
        assert!(
            next_retained.get() <= self.limits.retained_bytes().get(),
            "restored event must fit its previously owned bytes"
        );
        assert!(
            self.events
                .insert((token.at(), token.id()), Entry { event, retained })
                .is_none(),
            "restored event identity must be vacant"
        );
        self.retained = next_retained;
    }
}
