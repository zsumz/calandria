//! Bounded generated-operation differential model for resource ownership.

use std::{collections::BTreeMap, num::NonZeroUsize};

use calandria::{ResourceAdmissionFailure, ResourceOwnerId, ResourceTable, ResourceToken};

const CAPACITY: usize = 8;
const IDENTITIES: u8 = 12;
const SEEDS: u64 = 32;
const OPERATIONS: usize = 192;

#[test]
fn generated_resource_operations_match_a_simple_reference_owner() {
    for seed in 0..SEEDS {
        exercise_seed(seed);
    }
}

fn exercise_seed(seed: u64) {
    let owner = ResourceOwnerId::new(seed + 100);
    let capacity = nonzero(CAPACITY);
    let mut table = ResourceTable::new(owner, capacity);
    let mut active = BTreeMap::<ResourceToken, (u8, u64)>::new();
    let mut issued = Vec::<ResourceToken>::new();
    let mut generator = Generator::new(seed);

    for operation in 0..OPERATIONS {
        match generator.next() % 4 {
            0 => admit(
                &mut table,
                &mut active,
                &mut issued,
                operation,
                &mut generator,
            ),
            1 => remove(&mut table, &mut active, &issued, &mut generator),
            2 => mutate(&mut table, &mut active, &mut generator),
            _ => probe(&table, &active, &issued, &mut generator),
        }
        assert_state(&table, &active, owner, capacity);
    }
}

fn admit(
    table: &mut ResourceTable<u8, u64>,
    active: &mut BTreeMap<ResourceToken, (u8, u64)>,
    issued: &mut Vec<ResourceToken>,
    operation: usize,
    generator: &mut Generator,
) {
    let identity = u8::try_from(generator.next() % u64::from(IDENTITIES))
        .unwrap_or_else(|_| panic!("bounded identity must fit u8"));
    let resource = u64::try_from(operation).unwrap_or_else(|_| panic!("operation must fit u64"));
    let duplicate = active.values().any(|(current, _)| *current == identity);
    let result = table.admit(identity, resource);

    if duplicate {
        let error = result.unwrap_err_or_else(|| panic!("duplicate identity must reject"));
        assert_eq!(error.failure(), ResourceAdmissionFailure::IdentityInUse);
        assert!(!error.to_string().is_empty());
        assert_eq!(error.into_values(), (identity, resource));
    } else if active.len() == CAPACITY {
        let error = result.unwrap_err_or_else(|| panic!("full resource table must reject"));
        let (actual_identity, actual_resource, failure) = error.into_parts();
        assert_eq!((actual_identity, actual_resource), (identity, resource));
        assert_eq!(
            failure,
            ResourceAdmissionFailure::CapacityReached {
                limit: nonzero(CAPACITY),
            }
        );
    } else {
        let token = result.unwrap_or_else(|error| panic!("modeled resource must fit: {error}"));
        assert!(active.insert(token, (identity, resource)).is_none());
        issued.push(token);
    }
}

fn remove(
    table: &mut ResourceTable<u8, u64>,
    active: &mut BTreeMap<ResourceToken, (u8, u64)>,
    issued: &[ResourceToken],
    generator: &mut Generator,
) {
    let Some(token) = choose(issued, generator) else {
        assert!(table.is_empty());
        return;
    };
    let expected = active.remove(&token);
    match (table.remove(token), expected) {
        (Ok(actual), Some(expected)) => assert_eq!(actual, expected),
        (Err(failure), None) => assert!(!failure.to_string().is_empty()),
        (actual, expected) => panic!("resource/reference remove mismatch: {actual:?} {expected:?}"),
    }
}

fn mutate(
    table: &mut ResourceTable<u8, u64>,
    active: &mut BTreeMap<ResourceToken, (u8, u64)>,
    generator: &mut Generator,
) {
    let Some(token) = choose_map(active, generator) else {
        return;
    };
    let (expected_identity, expected_resource) = active
        .get_mut(&token)
        .unwrap_or_else(|| panic!("chosen reference resource must remain active"));
    let (identity, resource) = table
        .get_mut(token)
        .unwrap_or_else(|error| panic!("live token must resolve mutably: {error}"));
    assert_eq!(identity, expected_identity);
    *resource = resource.saturating_add(1);
    *expected_resource = expected_resource.saturating_add(1);
}

fn probe(
    table: &ResourceTable<u8, u64>,
    active: &BTreeMap<ResourceToken, (u8, u64)>,
    issued: &[ResourceToken],
    generator: &mut Generator,
) {
    let Some(token) = choose(issued, generator) else {
        return;
    };
    match (table.get(token), active.get(&token)) {
        (Ok((identity, resource)), Some((expected_identity, expected_resource))) => {
            assert_eq!((identity, resource), (expected_identity, expected_resource));
        }
        (Err(failure), None) => assert!(!failure.to_string().is_empty()),
        (actual, expected) => panic!("resource/reference lookup mismatch: {actual:?} {expected:?}"),
    }
}

fn assert_state(
    table: &ResourceTable<u8, u64>,
    active: &BTreeMap<ResourceToken, (u8, u64)>,
    owner: ResourceOwnerId,
    capacity: NonZeroUsize,
) {
    assert_eq!(table.owner(), owner);
    assert_eq!(table.capacity(), capacity);
    assert_eq!(table.len(), active.len());
    assert_eq!(table.is_empty(), active.is_empty());
    for identity in 0..IDENTITIES {
        let expected = active
            .iter()
            .find_map(|(token, (current, _))| (*current == identity).then_some(*token));
        assert_eq!(table.contains_identity(&identity), expected.is_some());
        assert_eq!(table.token_for(&identity), expected);
    }
    for (token, (identity, resource)) in active {
        assert_eq!(table.get(*token), Ok((identity, resource)));
    }
    let snapshot = table.snapshot();
    assert_eq!(snapshot.owner(), owner);
    assert_eq!(snapshot.capacity(), capacity);
    assert_eq!(snapshot.active(), active.len());
    assert_eq!(snapshot.vacant(), CAPACITY - active.len());
    assert_eq!(snapshot.exhausted(), 0);
}

fn choose<T: Copy>(values: &[T], generator: &mut Generator) -> Option<T> {
    let length = u64::try_from(values.len()).unwrap_or_else(|_| panic!("length must fit u64"));
    if length == 0 {
        return None;
    }
    let index = usize::try_from(generator.next() % length)
        .unwrap_or_else(|_| panic!("bounded index must fit usize"));
    values.get(index).copied()
}

fn choose_map(
    values: &BTreeMap<ResourceToken, (u8, u64)>,
    generator: &mut Generator,
) -> Option<ResourceToken> {
    let length = u64::try_from(values.len()).unwrap_or_else(|_| panic!("length must fit u64"));
    if length == 0 {
        return None;
    }
    let index = usize::try_from(generator.next() % length)
        .unwrap_or_else(|_| panic!("bounded index must fit usize"));
    values.keys().nth(index).copied()
}

#[derive(Debug)]
struct Generator(u64);

impl Generator {
    const fn new(seed: u64) -> Self {
        Self(seed ^ 0xe703_7ed1_a0b4_28db)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(2_862_933_555_777_941_757)
            .wrapping_add(3_037_000_493);
        self.0
    }
}

trait ResultExt<T, E> {
    fn unwrap_err_or_else(self, on_ok: impl FnOnce() -> E) -> E;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn unwrap_err_or_else(self, on_ok: impl FnOnce() -> E) -> E {
        match self {
            Ok(_) => on_ok(),
            Err(error) => error,
        }
    }
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test capacity must be nonzero"))
}
