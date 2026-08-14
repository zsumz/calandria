use core::convert::Infallible;

use calandria::{
    Deadline, Duty, EmbeddedHost, HostAction, HostConfig, Moment, Span, Turn, WorkCount,
};
use calandria_sim::VirtualClock;

#[derive(Debug)]
struct MaintenanceDuty {
    turns: u64,
}

impl Duty for MaintenanceDuty {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        self.turns = self.turns.saturating_add(1);
        Ok(Turn::until(
            WorkCount::new(1),
            Deadline::at(Moment::from_nanos(10)),
        ))
    }
}

#[test]
fn production_host_contract_runs_against_virtual_time() {
    let duty = MaintenanceDuty { turns: 0 };
    let clock = VirtualClock::at(Moment::from_nanos(7));
    let mut host = EmbeddedHost::new(duty, clock, HostConfig::new(Span::from_nanos(100)));

    let step = host
        .step()
        .unwrap_or_else(|error| panic!("virtual host step failed: {error}"));

    assert_eq!(step.started_at(), Moment::from_nanos(7));
    assert_eq!(step.completed_at(), Moment::from_nanos(7));
    assert_eq!(step.action(), HostAction::Wait(Span::from_nanos(3)));
    assert_eq!(host.duty().turns, 1);
}
