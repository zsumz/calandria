//! Vacant, occupied, and permanently retired resource slots.

use super::{ResourceGeneration, ResourceToken, ResourceTokenFailure};

#[derive(Debug)]
pub(super) enum Slot<K, R> {
    Vacant {
        generation: ResourceGeneration,
    },
    Occupied {
        generation: ResourceGeneration,
        identity: K,
        resource: R,
    },
    Exhausted,
}

pub(super) fn token_failure<K, R>(
    slot: &Slot<K, R>,
    token: ResourceToken,
) -> ResourceTokenFailure {
    match slot {
        Slot::Vacant { generation } => ResourceTokenFailure::Vacant {
            slot: token.slot(),
            generation: *generation,
        },
        Slot::Occupied { generation, .. } => ResourceTokenFailure::GenerationMismatch {
            slot: token.slot(),
            current: *generation,
            supplied: token.generation(),
        },
        Slot::Exhausted => ResourceTokenFailure::Exhausted { slot: token.slot() },
    }
}
