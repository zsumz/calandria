//! Reactor-group topology rejection and thread-start ownership tests.

use std::error::Error;

use calandria::{ReactorGroup, ReactorGroupSpawnFailure, ReactorId};

use super::support::{FirstTurn, Member, group_limits, member, observations};

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
