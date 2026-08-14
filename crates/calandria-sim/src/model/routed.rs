//! Internal target and causal metadata retained beside modeled events.

use calandria::{Retained, RetainedBytes};

use crate::ActionId;

use super::DutyId;

#[derive(Debug)]
pub(crate) struct Routed<E> {
    pub(crate) target: DutyId,
    pub(crate) cause: Option<ActionId>,
    pub(crate) event: E,
}

impl<E> Routed<E> {
    pub(crate) const fn new(
        target: DutyId,
        cause: Option<ActionId>,
        event: E,
    ) -> Self {
        Self {
            target,
            cause,
            event,
        }
    }
}

impl<E: Retained> Retained for Routed<E> {
    fn retained_bytes(&self) -> RetainedBytes {
        self.event.retained_bytes()
    }
}
