//! Reactor-group public ingress, materialization, and observation API tests.

use std::error::Error;

use calandria::{
    AdmissionFailure, Lane, ReactorGroup, ReactorGroupHandle, ReactorGroupOutcome,
    ReactorGroupSendFailure, ReactorGroupSpawnFailure, ReactorId, RetainedBytes,
};

#[path = "reactor_group_api_support/mod.rs"]
mod support;

use support::{Command, group_limits, measured_member, member, observations, observed_values};

#[test]
fn group_accessors_and_ingress_preserve_topology_and_ownership() -> Result<(), Box<dyn Error>> {
    let observed = observations();
    let limits = group_limits(1);
    let group = ReactorGroup::spawn(
        limits,
        "calandria-group-api",
        vec![member(ReactorId::new(0), &observed)],
    )?;

    assert_eq!(group.limits(), limits);
    assert_eq!(group.limits().reactors().get(), 1);
    assert_eq!(group.reactors().get(), 1);
    assert!(group.reactor_thread(ReactorId::new(0)).is_some());
    assert!(group.reactor_thread(ReactorId::new(u64::MAX)).is_none());
    assert!(!group.is_finished());
    assert!(format!("{group:?}").contains("ReactorGroup"));

    let ingress = group.handle();
    assert_eq!(ingress.reactors().get(), 1);
    assert!(format!("{ingress:?}").contains("ReactorGroupHandle"));
    let snapshot = ingress
        .mailbox_snapshot(ReactorId::new(0))
        .unwrap_or_else(|| panic!("known reactor must expose a mailbox snapshot"));
    assert_eq!(snapshot.lane(Lane::Control).limits().messages().get(), 2);
    assert_eq!(snapshot.lane(Lane::Work).limits().messages().get(), 8);
    assert!(ingress.mailbox_snapshot(ReactorId::new(u64::MAX)).is_none());

    let Err(retained) = ingress.try_send_to(ReactorId::new(0), Lane::Work, Command::Retained(98))
    else {
        panic!("retained command must exceed the zero-byte mailbox limit");
    };
    assert!(matches!(
        retained.failure(),
        ReactorGroupSendFailure::Mailbox(AdmissionFailure::ByteCapacity)
    ));
    assert_eq!(retained.into_item(), Command::Retained(98));

    let Err(unknown) = ingress.try_send_control(ReactorId::new(u64::MAX), Command::Record(99))
    else {
        panic!("unknown reactor must reject control ingress");
    };
    assert_eq!(unknown.lane(), Lane::Control);
    assert!(!unknown.to_string().is_empty());
    let (item, lane, failure) = unknown.into_parts();
    assert_eq!(item, Command::Record(99));
    assert_eq!(lane, Lane::Control);
    assert!(matches!(
        failure,
        ReactorGroupSendFailure::UnknownReactor { reactor, reactors }
            if reactor == ReactorId::new(u64::MAX) && reactors.get() == 1
    ));

    let mut materialized = false;
    let Err(bytes) = ingress.try_send_materialized(
        ReactorId::new(0),
        Lane::Work,
        7_u64,
        |_| RetainedBytes::new(1),
        |value| {
            materialized = true;
            Command::Record(value)
        },
    ) else {
        panic!("zero-byte mailbox must reject retained materialization");
    };
    assert!(!materialized);
    assert_eq!(bytes.into_item(), 7);

    ingress.try_send_control(ReactorId::new(0), Command::Record(10))?;
    let mut materialized = false;
    ingress.try_send_materialized(
        ReactorId::new(0),
        Lane::Work,
        20_u64,
        |_| RetainedBytes::ZERO,
        |value| {
            materialized = true;
            Command::Record(value)
        },
    )?;
    assert!(materialized);
    ingress.try_send_to(ReactorId::new(0), Lane::Work, Command::Stop)?;

    let exit = group
        .join()
        .unwrap_or_else(|_| panic!("reactor group supervisor panicked"));
    assert_eq!(exit.outcome(), ReactorGroupOutcome::Stopped);
    assert_eq!(
        observed_values(&observed),
        [(ReactorId::new(0), 10), (ReactorId::new(0), 20)]
    );
    assert!(!ingress.is_open());
    assert_closed_materialization(&ingress);
    Ok(())
}

fn assert_closed_materialization(ingress: &ReactorGroupHandle<Command>) {
    let mut retained_measured = false;
    let mut materialized = false;
    let Err(closed) = ingress.try_send_materialized(
        ReactorId::new(0),
        Lane::Work,
        30_u64,
        |_| {
            retained_measured = true;
            RetainedBytes::ZERO
        },
        |_| {
            materialized = true;
            Command::Stop
        },
    ) else {
        panic!("terminal group must reject before materialization");
    };
    assert!(!retained_measured);
    assert!(!materialized);
    assert!(matches!(closed.failure(), ReactorGroupSendFailure::Closed));
    assert_eq!(
        closed.to_string(),
        "reactor group admission is closed in Work lane"
    );
    assert!(Error::source(&closed).is_none());
    assert_eq!(closed.into_item(), 30);
}

#[test]
fn measured_member_exposes_its_unstarted_reactor() {
    let observed = observations();
    let member = measured_member(ReactorId::new(7), &observed);
    assert_eq!(member.id(), ReactorId::new(7));
    assert!(format!("{:?}", member.reactor()).contains("Reactor"));
}

#[test]
fn member_identity_mismatch_rejects_before_start_and_returns_ownership() {
    let observed = observations();
    let members = vec![
        member(ReactorId::new(0), &observed),
        member(ReactorId::new(7), &observed),
        member(ReactorId::new(2), &observed),
    ];
    let Err(error) = ReactorGroup::spawn(group_limits(3), "identity-mismatch", members) else {
        panic!("mismatched member identity must reject startup");
    };
    assert!(matches!(
        error.failure(),
        ReactorGroupSpawnFailure::Identity { expected, actual }
            if *expected == ReactorId::new(1) && *actual == ReactorId::new(7)
    ));
    assert_eq!(
        error.to_string(),
        "reactor group member at position 1 declared identity 7"
    );
    assert!(Error::source(&error).is_none());
    let (failure, members) = error.into_parts();
    assert!(matches!(failure, ReactorGroupSpawnFailure::Identity { .. }));
    assert_eq!(
        members
            .iter()
            .map(calandria::ReactorGroupMember::id)
            .collect::<Vec<_>>(),
        [ReactorId::new(0), ReactorId::new(7), ReactorId::new(2)]
    );
}

#[test]
fn empty_topology_into_members_returns_the_exact_empty_owner_set() {
    let members: Vec<support::Member> = Vec::new();
    let Err(error) = ReactorGroup::spawn(group_limits(1), "empty-api", members) else {
        panic!("empty group must reject startup");
    };
    assert!(matches!(error.failure(), ReactorGroupSpawnFailure::Empty));
    assert!(error.into_members().is_empty());
}

#[test]
fn materialized_byte_rejection_maps_mailbox_failure_and_preserves_owner()
-> Result<(), Box<dyn Error>> {
    let observed = observations();
    let group = ReactorGroup::spawn(
        group_limits(1),
        "calandria-group-rejection",
        vec![member(ReactorId::new(0), &observed)],
    )?;
    let ingress = group.handle();

    let Err(error) = ingress.try_send_materialized(
        ReactorId::new(0),
        Lane::Control,
        String::from("owned"),
        |_| RetainedBytes::new(1),
        |_| panic!("rejected owner must not be materialized"),
    ) else {
        panic!("retained control value must exceed the zero-byte limit");
    };
    assert_eq!(error.lane(), Lane::Control);
    assert!(matches!(
        error.failure(),
        ReactorGroupSendFailure::Mailbox(AdmissionFailure::ByteCapacity)
    ));
    assert!(
        error
            .to_string()
            .contains("reactor mailbox rejected ingress")
    );
    assert!(Error::source(&error).is_some());
    assert_eq!(error.into_item(), "owned");

    ingress.try_send(ReactorId::new(0), Command::Stop)?;
    let exit = group
        .join()
        .unwrap_or_else(|_| panic!("reactor group supervisor panicked"));
    assert_eq!(exit.outcome(), ReactorGroupOutcome::Stopped);
    Ok(())
}
