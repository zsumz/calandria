//! Static reactor-group routing, termination, and startup tests.

use std::{error::Error, sync::mpsc, time::Duration};

use calandria::{
    Lane, ReactorFailure, ReactorGroup, ReactorGroupMemberExit, ReactorGroupOutcome,
    ReactorGroupSendFailure, ReactorGroupSpawnFailure, ReactorId, ReactorOutcome,
    ReactorTerminationStatus, RetainedBytes, WakeHandle,
};

#[path = "reactor_group_support/mod.rs"]
mod support;

use support::{
    Command, FirstTurn, Member, group_limits, member, member_with_termination_wake, observations,
    observed_values,
};

#[test]
fn one_duty_type_runs_as_an_ordered_shared_nothing_group() -> Result<(), Box<dyn Error>> {
    let observed = observations();
    let members = vec![
        member(ReactorId::new(0), &observed, FirstTurn::Run),
        member(ReactorId::new(1), &observed, FirstTurn::Run),
    ];
    let group = ReactorGroup::spawn(group_limits(2), "calandria-group", members)?;
    let ingress = group.handle();

    ingress.try_send(ReactorId::new(0), Command::Record(10))?;
    ingress.try_send(ReactorId::new(1), Command::Record(20))?;
    let mut materialized = false;
    let Err(invalid) = ingress.try_send_materialized(
        ReactorId::new(u64::MAX),
        Lane::Work,
        30,
        |_| RetainedBytes::ZERO,
        |value| {
            materialized = true;
            Command::Record(value)
        },
    ) else {
        panic!("unknown reactor must reject ingress");
    };
    assert!(!materialized);
    assert!(matches!(
        invalid.failure(),
        ReactorGroupSendFailure::UnknownReactor { reactor, .. }
            if *reactor == ReactorId::new(u64::MAX)
    ));
    assert_eq!(invalid.into_item(), 30);
    ingress.try_send(ReactorId::new(0), Command::Stop)?;
    ingress.try_send(ReactorId::new(1), Command::Stop)?;

    let exit = group
        .join()
        .unwrap_or_else(|_| panic!("reactor group supervisor panicked"));
    assert_eq!(exit.outcome(), ReactorGroupOutcome::Stopped);
    assert!(!ingress.is_open());
    assert_eq!(exit.len(), 2);
    assert!(!exit.is_empty());
    assert!(exit.member(ReactorId::new(u64::MAX)).is_none());
    assert!(format!("{exit:?}").contains("ReactorGroupExit"));
    for (id, member) in exit.members() {
        let ReactorGroupMemberExit::Exited(member) = member else {
            panic!("member {id:?} panicked");
        };
        assert!(matches!(member.outcome(), ReactorOutcome::Stopped));
        assert_eq!(member.duty().id(), id);
    }
    let mut actual = observed_values(&observed);
    actual.sort_unstable();
    assert_eq!(
        actual,
        vec![(ReactorId::new(0), 10), (ReactorId::new(1), 20)]
    );
    let Err(closed) = ingress.try_send(ReactorId::new(0), Command::Record(40)) else {
        panic!("terminal group must close ingress");
    };
    assert!(matches!(closed.failure(), ReactorGroupSendFailure::Closed));
    for (index, member) in exit.into_members().into_vec().into_iter().enumerate() {
        let terminal = member
            .into_result()
            .unwrap_or_else(|_| panic!("stopped member must retain its reactor exit"));
        assert_eq!(terminal.into_duty().id(), ReactorId::new(index as u64));
    }
    Ok(())
}

#[test]
fn fatal_member_failure_closes_ingress_and_terminates_peers() -> Result<(), Box<dyn Error>> {
    let observed = observations();
    let members = vec![
        member(ReactorId::new(0), &observed, FirstTurn::Fail),
        member(ReactorId::new(1), &observed, FirstTurn::Run),
    ];
    let group = ReactorGroup::spawn(group_limits(2), "calandria-failing-group", members)?;
    let ingress = group.handle();
    let exit = group
        .join()
        .unwrap_or_else(|_| panic!("failing group supervisor panicked"));

    assert_eq!(
        exit.outcome(),
        ReactorGroupOutcome::Failed(ReactorId::new(0))
    );
    assert!(!ingress.is_open());
    let ReactorGroupMemberExit::Exited(failed) = exit
        .member(ReactorId::new(0))
        .unwrap_or_else(|| panic!("failed member must be retained"))
    else {
        panic!("failed member unexpectedly panicked");
    };
    assert!(matches!(
        failed.outcome(),
        ReactorOutcome::Failed(ReactorFailure::Host(_))
    ));
    let ReactorGroupMemberExit::Exited(peer) = exit
        .member(ReactorId::new(1))
        .unwrap_or_else(|| panic!("peer member must be retained"))
    else {
        panic!("peer member unexpectedly panicked");
    };
    assert!(matches!(peer.outcome(), ReactorOutcome::Terminated));
    Ok(())
}

#[test]
fn panicking_member_closes_ingress_and_terminates_peers() -> Result<(), Box<dyn Error>> {
    let observed = observations();
    let members = vec![
        member(ReactorId::new(0), &observed, FirstTurn::Panic),
        member(ReactorId::new(1), &observed, FirstTurn::Run),
    ];
    let group = ReactorGroup::spawn(group_limits(2), "calandria-panicking-group", members)?;
    let ingress = group.handle();
    let exit = group
        .join()
        .unwrap_or_else(|_| panic!("panicking group supervisor panicked"));

    assert_eq!(
        exit.outcome(),
        ReactorGroupOutcome::Panicked(ReactorId::new(0))
    );
    assert!(!ingress.is_open());
    assert!(matches!(
        exit.member(ReactorId::new(0)),
        Some(ReactorGroupMemberExit::Panicked(_))
    ));
    assert!(format!("{:?}", exit.member(ReactorId::new(0))).contains("Panicked"));
    let Some(ReactorGroupMemberExit::Exited(peer)) = exit.member(ReactorId::new(1)) else {
        panic!("peer member must return an owned exit");
    };
    assert!(matches!(peer.outcome(), ReactorOutcome::Terminated));

    let mut members = exit.into_members().into_vec().into_iter();
    let panicked = members
        .next()
        .unwrap_or_else(|| panic!("panicking member must be retained"));
    let Err(payload) = panicked.into_result() else {
        panic!("panicking member must preserve its panic payload");
    };
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"planned reactor panic")
    );
    assert!(
        members
            .next()
            .unwrap_or_else(|| panic!("peer member must be retained"))
            .into_result()
            .is_ok()
    );
    Ok(())
}

#[test]
fn panicking_termination_wake_cannot_strand_later_peers() -> Result<(), Box<dyn Error>> {
    let observed = observations();
    let members = vec![
        member(ReactorId::new(0), &observed, FirstTurn::Fail),
        member_with_termination_wake(
            ReactorId::new(1),
            &observed,
            FirstTurn::Run,
            WakeHandle::new(|| panic!("planned termination wake panic")),
        ),
        member(ReactorId::new(2), &observed, FirstTurn::Run),
    ];
    let group = ReactorGroup::spawn(group_limits(3), "calandria-wake-panic-group", members)?;
    let (joined, terminal) = mpsc::sync_channel(0);
    std::thread::spawn(move || {
        let exit = group
            .join()
            .unwrap_or_else(|_| panic!("wake-panic group supervisor panicked"));
        let _ = joined.send(exit);
    });
    let exit = terminal.recv_timeout(Duration::from_secs(2))?;

    assert_eq!(
        exit.outcome(),
        ReactorGroupOutcome::Panicked(ReactorId::new(0))
    );
    for reactor in [ReactorId::new(1), ReactorId::new(2)] {
        let Some(ReactorGroupMemberExit::Exited(peer)) = exit.member(reactor) else {
            panic!("peer {reactor:?} must return an owned exit");
        };
        assert!(matches!(peer.outcome(), ReactorOutcome::Terminated));
    }
    Ok(())
}

#[test]
fn explicit_group_termination_is_bounded_and_identity_ordered() -> Result<(), Box<dyn Error>> {
    let observed = observations();
    let members = vec![
        member(ReactorId::new(0), &observed, FirstTurn::Run),
        member(ReactorId::new(1), &observed, FirstTurn::Run),
    ];
    let group = ReactorGroup::spawn(group_limits(2), "calandria-terminated-group", members)?;

    let termination = group.request_termination();
    assert_eq!(termination.len(), 2);
    assert!(!termination.is_empty());
    assert!(termination.get(ReactorId::new(0)).is_some());
    assert!(termination.get(ReactorId::new(u64::MAX)).is_none());
    assert!(format!("{termination:?}").contains("ReactorGroupTermination"));
    for (id, result) in termination.iter() {
        assert!(id.get() < 2);
        assert_eq!(result.status(), ReactorTerminationStatus::Requested);
    }
    let exit = group
        .join()
        .unwrap_or_else(|_| panic!("terminated group supervisor panicked"));
    assert_eq!(exit.outcome(), ReactorGroupOutcome::Terminated);
    Ok(())
}

#[test]
fn topology_rejection_and_thread_failure_return_every_unstarted_member() {
    let observed = observations();
    let empty: Vec<Member> = Vec::new();
    let Err(error) = ReactorGroup::spawn(group_limits(1), "empty", empty) else {
        panic!("empty topology must reject startup");
    };
    assert!(matches!(error.failure(), ReactorGroupSpawnFailure::Empty));
    assert_eq!(error.to_string(), "reactor group topology is empty");
    assert!(format!("{error:?}").contains("member_count: 0"));
    assert!(Error::source(&error).is_none());
    let (failure, members) = error.into_parts();
    assert!(matches!(failure, ReactorGroupSpawnFailure::Empty));
    assert!(members.is_empty());
    let members = vec![
        member(ReactorId::new(0), &observed, FirstTurn::Run),
        member(ReactorId::new(1), &observed, FirstTurn::Run),
    ];
    let Err(error) = ReactorGroup::spawn(group_limits(1), "too-large", members) else {
        panic!("topology above its limit must reject startup");
    };
    assert!(matches!(
        error.failure(),
        ReactorGroupSpawnFailure::Capacity { actual: 2, .. }
    ));
    assert_eq!(
        error.to_string(),
        "reactor group has 2 members but limit is 1"
    );
    assert!(Error::source(&error).is_none());
    let (failure, members) = error.into_parts();
    assert!(matches!(
        failure,
        ReactorGroupSpawnFailure::Capacity { actual: 2, .. }
    ));
    assert_eq!(members.len(), 2);

    let Err(error) = ReactorGroup::spawn(group_limits(2), "invalid\0group", members) else {
        panic!("invalid thread name must reject group startup");
    };
    assert!(matches!(
        error.failure(),
        ReactorGroupSpawnFailure::ReactorThread { reactor, .. }
            if *reactor == ReactorId::new(0)
    ));
    assert!(
        error
            .to_string()
            .contains("reactor 0 thread creation failed")
    );
    assert!(Error::source(&error).is_some());
    let (failure, members) = error.into_parts();
    assert!(matches!(
        failure,
        ReactorGroupSpawnFailure::ReactorThread { reactor, .. }
            if reactor == ReactorId::new(0)
    ));
    assert_eq!(members.len(), 2);

    let supervisor = ReactorGroupSpawnFailure::SupervisorThread {
        source: std::io::Error::other("planned supervisor failure"),
    };
    assert_eq!(
        supervisor.to_string(),
        "reactor group supervisor creation failed: planned supervisor failure"
    );
}
