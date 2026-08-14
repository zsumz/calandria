//! Transactional exact cancellation with rollback ownership.

use calandria::Retained;

use crate::EventToken;

use super::ActionContext;
use crate::model::CancelFailure;

impl<E: Retained, O: Retained> ActionContext<'_, E, O> {
    /// Cancels one exact pending event transactionally.
    pub fn cancel(&mut self, token: EventToken) -> Result<(), CancelFailure> {
        if self.effects >= self.limits.effects.get() {
            return Err(CancelFailure::EffectCapacity {
                limit: self.limits.effects,
            });
        }
        if token.timeline() != self.timeline.id() {
            return Err(CancelFailure::ForeignTimeline {
                expected: self.timeline.id(),
                actual: token.timeline(),
            });
        }
        let Some(retained) = self.timeline.retained_for(token) else {
            return Err(CancelFailure::NotPending(token));
        };

        if self.inserted.remove(&token).is_some() {
            let removed = self
                .timeline
                .cancel(token)
                .unwrap_or_else(|| panic!("inserted event must remain pending"));
            drop(removed);
            self.effect_bytes = self
                .effect_bytes
                .checked_sub(retained)
                .unwrap_or_else(|| panic!("inserted effect accounting must be exact"));
            self.effects += 1;
            return Ok(());
        }

        let Some(next_bytes) = self.effect_bytes.checked_add(retained) else {
            return Err(CancelFailure::RetainedByteOverflow {
                current: self.effect_bytes,
                event: retained,
            });
        };
        if next_bytes > self.limits.effect_bytes {
            return Err(CancelFailure::RetainedByteCapacity {
                limit: self.limits.effect_bytes,
                current: self.effect_bytes,
                event: retained,
            });
        }
        let removed = self
            .timeline
            .cancel(token)
            .unwrap_or_else(|| panic!("validated pending event must be removable"));
        self.removed.push((token, removed));
        self.effect_bytes = next_bytes;
        self.effects += 1;
        Ok(())
    }
}
