//! Bounded structured observations committed beside successful actions.

use calandria::Retained;

use super::ActionContext;
use crate::model::{ObservationError, ObservationFailure};

impl<E: Retained, O: Retained> ActionContext<'_, E, O> {
    /// Emits one bounded structured observation for the action record.
    pub fn observe(&mut self, observation: O) -> Result<(), ObservationError<O>> {
        if self.observations.len() >= self.limits.observations.get() {
            return Err(ObservationError::new(
                observation,
                ObservationFailure::Capacity {
                    limit: self.limits.observations,
                },
            ));
        }
        let retained = observation.retained_bytes();
        let Some(next_bytes) = self.observation_bytes.checked_add(retained) else {
            return Err(ObservationError::new(
                observation,
                ObservationFailure::RetainedByteOverflow {
                    current: self.observation_bytes,
                    observation: retained,
                },
            ));
        };
        if next_bytes > self.limits.observation_bytes {
            return Err(ObservationError::new(
                observation,
                ObservationFailure::RetainedByteCapacity {
                    limit: self.limits.observation_bytes,
                    current: self.observation_bytes,
                    observation: retained,
                },
            ));
        }
        self.observations.push(observation);
        self.observation_bytes = next_bytes;
        Ok(())
    }
}
