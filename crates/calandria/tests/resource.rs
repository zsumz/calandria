//! Generational resource-table ownership and stale-token tests.

use std::num::NonZeroUsize;

use calandria::{
    ResourceAdmissionFailure, ResourceGeneration, ResourceOwnerId, ResourceSlotId, ResourceTable,
    ResourceToken, ResourceTokenFailure,
};

#[test]
fn admission_is_bounded_unique_and_ownership_preserving() {
    let owner = ResourceOwnerId::new(7);
    let mut table = ResourceTable::new(owner, nonzero(1));
    let token = table
        .admit("first", String::from("socket-1"))
        .unwrap_or_else(|error| panic!("first resource must fit: {error}"));

    let Err(duplicate) = table.admit("first", String::from("socket-2")) else {
        panic!("live identities must be unique");
    };
    assert_eq!(duplicate.failure(), ResourceAdmissionFailure::IdentityInUse);
    assert_eq!(duplicate.into_values(), ("first", String::from("socket-2")));

    let Err(full) = table.admit("second", String::from("socket-3")) else {
        panic!("second identity must exceed capacity");
    };
    assert_eq!(
        full.failure(),
        ResourceAdmissionFailure::CapacityReached { limit: nonzero(1) }
    );
    assert_eq!(full.into_values(), ("second", String::from("socket-3")));
    assert_eq!(table.len(), 1);
    assert_eq!(table.token_for(&"first"), Some(token));
}

#[test]
fn slot_reuse_rejects_stale_generation_without_touching_current_resource() {
    let owner = ResourceOwnerId::new(11);
    let mut table = ResourceTable::new(owner, nonzero(1));
    let stale = table
        .admit(1_u64, String::from("old"))
        .unwrap_or_else(|error| panic!("old resource must fit: {error}"));

    assert_eq!(table.remove(stale), Ok((1, String::from("old"))));
    assert_eq!(
        table.get(stale),
        Err(ResourceTokenFailure::Vacant {
            slot: stale.slot(),
            generation: ResourceGeneration::new(1),
        })
    );

    let current = table
        .admit(2_u64, String::from("new"))
        .unwrap_or_else(|error| panic!("reused slot must fit: {error}"));
    assert_eq!(current.slot(), stale.slot());
    assert_eq!(current.generation(), ResourceGeneration::new(1));
    assert_eq!(
        table.get_mut(stale).map(|_| ()),
        Err(ResourceTokenFailure::GenerationMismatch {
            slot: stale.slot(),
            current: ResourceGeneration::new(1),
            supplied: ResourceGeneration::INITIAL,
        })
    );
    assert_eq!(
        table.remove(stale),
        Err(ResourceTokenFailure::GenerationMismatch {
            slot: stale.slot(),
            current: ResourceGeneration::new(1),
            supplied: ResourceGeneration::INITIAL,
        })
    );
    assert_eq!(
        table.get(current).map(|(_, value)| value.as_str()),
        Ok("new")
    );
}

#[test]
fn owner_and_slot_are_validated_before_generation() {
    let owner = ResourceOwnerId::new(1);
    let mut table = ResourceTable::new(owner, nonzero(1));
    let token = table
        .admit("identity", 5_u8)
        .unwrap_or_else(|error| panic!("resource must fit: {error}"));

    let foreign = ResourceToken::new(ResourceOwnerId::new(2), token.slot(), token.generation());
    assert_eq!(
        table.get(foreign),
        Err(ResourceTokenFailure::OwnerMismatch {
            expected: owner,
            actual: ResourceOwnerId::new(2),
        })
    );

    let out_of_bounds = ResourceToken::new(
        owner,
        ResourceSlotId::new(u64::MAX),
        ResourceGeneration::INITIAL,
    );
    assert_eq!(
        table.get(out_of_bounds),
        Err(ResourceTokenFailure::SlotOutOfBounds {
            slot: ResourceSlotId::new(u64::MAX),
            capacity: nonzero(1),
        })
    );
    assert_eq!(table.get(token), Ok((&"identity", &5_u8)));
}

#[test]
fn mutable_resolution_requires_the_exact_live_token() {
    let mut table = ResourceTable::new(ResourceOwnerId::new(9), nonzero(2));
    let token = table
        .admit(String::from("worker"), 10_u64)
        .unwrap_or_else(|error| panic!("resource must fit: {error}"));

    let (identity, resource) = table
        .get_mut(token)
        .unwrap_or_else(|error| panic!("live token must resolve: {error}"));
    assert_eq!(identity.as_str(), "worker");
    *resource = 44;

    assert_eq!(table.get(token).map(|(_, value)| *value), Ok(44));
    let snapshot = table.snapshot();
    assert_eq!(snapshot.owner(), ResourceOwnerId::new(9));
    assert_eq!(snapshot.active(), 1);
    assert_eq!(snapshot.vacant(), 1);
    assert_eq!(snapshot.exhausted(), 0);
}

#[test]
fn admission_selects_the_lowest_vacant_slot_deterministically() {
    let mut table = ResourceTable::new(ResourceOwnerId::new(13), nonzero(2));
    let first = table
        .admit("first", ())
        .unwrap_or_else(|error| panic!("first resource must fit: {error}"));
    let second = table
        .admit("second", ())
        .unwrap_or_else(|error| panic!("second resource must fit: {error}"));

    assert_eq!(first.slot(), ResourceSlotId::new(0));
    assert_eq!(second.slot(), ResourceSlotId::new(1));
    let _ = table
        .remove(first)
        .unwrap_or_else(|error| panic!("first resource must remove: {error}"));
    let replacement = table
        .admit("replacement", ())
        .unwrap_or_else(|error| panic!("vacated slot must be reused: {error}"));

    assert_eq!(replacement.slot(), ResourceSlotId::new(0));
    assert_eq!(replacement.generation(), ResourceGeneration::new(1));
}

#[test]
fn final_generation_retires_the_slot_permanently() {
    let owner = ResourceOwnerId::new(17);
    let mut table = ResourceTable::starting_at(owner, nonzero(1), ResourceGeneration::MAX);
    let last = table
        .admit("last", String::from("owned"))
        .unwrap_or_else(|error| panic!("last generation must fit: {error}"));

    assert_eq!(table.remove(last), Ok(("last", String::from("owned"))));
    assert_eq!(
        table.get(last),
        Err(ResourceTokenFailure::Exhausted { slot: last.slot() })
    );
    let snapshot = table.snapshot();
    assert_eq!(snapshot.active(), 0);
    assert_eq!(snapshot.vacant(), 0);
    assert_eq!(snapshot.exhausted(), 1);

    let Err(error) = table.admit("returned", String::from("resource")) else {
        panic!("exhausted slot must never be reused");
    };
    assert_eq!(
        error.failure(),
        ResourceAdmissionFailure::TokenSpaceExhausted
    );
    assert_eq!(error.into_values(), ("returned", String::from("resource")));
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test capacity must be nonzero"))
}
