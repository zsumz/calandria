//! Transactional immediate, delayed, and absolute event publication.

use calandria::{Moment, Retained, Span};

use crate::EventToken;

use super::ActionContext;
use crate::model::{DutyId, Routed, SendError, SendFailure};

impl<E: Retained, O: Retained> ActionContext<'_, E, O> {
    /// Schedules an immediate typed delivery.
    pub fn send(
        &mut self,
        target: DutyId,
        event: E,
    ) -> Result<EventToken, SendError<E>> {
        self.send_at(target, self.now, event)
    }

    /// Schedules a typed delivery after a relative virtual delay.
    pub fn send_after(
        &mut self,
        target: DutyId,
        delay: Span,
        event: E,
    ) -> Result<EventToken, SendError<E>> {
        let Some(at) = self.now.checked_add(delay) else {
            return Err(SendError::new(
                event,
                SendFailure::TimeOverflow {
                    current: self.now,
                    delay,
                },
            ));
        };
        self.send_at(target, at, event)
    }

    /// Schedules a typed delivery at an absolute virtual moment.
    pub fn send_at(
        &mut self,
        target: DutyId,
        at: Moment,
        event: E,
    ) -> Result<EventToken, SendError<E>> {
        self.validate_target(target, at, event)
    }

    fn validate_target(
        &mut self,
        target: DutyId,
        at: Moment,
        event: E,
    ) -> Result<EventToken, SendError<E>> {
        if self.effects >= self.limits.effects.get() {
            return Err(SendError::new(
                event,
                SendFailure::EffectCapacity {
                    limit: self.limits.effects,
                },
            ));
        }
        if !self.topology.contains(target) {
            return Err(SendError::new(
                event,
                SendFailure::UnknownTarget(target),
            ));
        }
        if self.stopped.contains(&target) {
            return Err(SendError::new(
                event,
                SendFailure::TargetStopped(target),
            ));
        }
        if at > self.limits.max_time {
            return Err(SendError::new(
                event,
                SendFailure::BeyondTimeLimit {
                    requested: at,
                    limit: self.limits.max_time,
                },
            ));
        }
        self.admit(target, at, event)
    }

    fn admit(
        &mut self,
        target: DutyId,
        at: Moment,
        event: E,
    ) -> Result<EventToken, SendError<E>> {
        let retained = event.retained_bytes();
        let Some(next_bytes) = self.effect_bytes.checked_add(retained) else {
            return Err(SendError::new(
                event,
                SendFailure::RetainedByteOverflow {
                    current: self.effect_bytes,
                    event: retained,
                },
            ));
        };
        if next_bytes > self.limits.effect_bytes {
            return Err(SendError::new(
                event,
                SendFailure::RetainedByteCapacity {
                    limit: self.limits.effect_bytes,
                    current: self.effect_bytes,
                    event: retained,
                },
            ));
        }

        let routed = Routed::new(target, Some(self.action), event);
        let token = match self.timeline.schedule_at(at, routed) {
            Ok(token) => token,
            Err(error) => {
                let (routed, failure) = error.into_parts();
                return Err(SendError::new(
                    routed.event,
                    SendFailure::Timeline(failure),
                ));
            }
        };
        self.effects += 1;
        self.effect_bytes = next_bytes;
        self.inserted.insert(token, retained);
        Ok(token)
    }
}
