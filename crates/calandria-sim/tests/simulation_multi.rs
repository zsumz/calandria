use core::convert::Infallible;

use calandria::{Moment, Retained, RetainedBytes, Turn, WorkCount};
use calandria_sim::{
    ActionContext, Delivery, DutyId, Model, RoundRobin, RunEnd, Simulation,
    SimulationLimits, TimelineId, Topology,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Message {
    Ping(u8),
    Stop,
}

impl Retained for Message {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::ZERO
    }
}

#[derive(Debug)]
struct PingPong {
    started: bool,
    deliveries: u8,
}

impl Model for PingPong {
    type Event = Message;
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        duty: DutyId,
        _now: Moment,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        if duty == DutyId::new(0) && !self.started {
            self.started = true;
            let _ = context
                .send(DutyId::new(1), Message::Ping(4))
                .unwrap_or_else(|error| panic!("initial ping must fit: {error}"));
        }
        Ok(Turn::waiting())
    }

    fn deliver(
        &mut self,
        duty: DutyId,
        delivery: Delivery<Self::Event>,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        self.deliveries = self.deliveries.saturating_add(1);
        let other = if duty == DutyId::new(0) {
            DutyId::new(1)
        } else {
            DutyId::new(0)
        };
        match delivery.into_event() {
            Message::Ping(0) => {
                let _ = context
                    .send(other, Message::Stop)
                    .unwrap_or_else(|error| panic!("terminal message must fit: {error}"));
                Ok(Turn::stopped(WorkCount::new(1)))
            }
            Message::Ping(remaining) => {
                let _ = context
                    .send(other, Message::Ping(remaining - 1))
                    .unwrap_or_else(|error| panic!("next ping must fit: {error}"));
                Ok(Turn::waiting())
            }
            Message::Stop => Ok(Turn::stopped(WorkCount::new(1))),
        }
    }
}

#[test]
fn two_independent_owners_exchange_typed_deliveries() {
    let topology = Topology::new([DutyId::new(0), DutyId::new(1)])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"));
    let mut simulation = Simulation::with_scheduler(
        TimelineId::new(21),
        PingPong {
            started: false,
            deliveries: 0,
        },
        topology,
        SimulationLimits::default(),
        RoundRobin::new(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));

    let report = simulation
        .run_to_completion()
        .unwrap_or_else(|error| panic!("ping pong must complete: {error}"));

    assert_eq!(report.end(), RunEnd::Completed);
    assert_eq!(report.snapshot().stopped(), 2);
    assert_eq!(report.snapshot().pending_events(), 0);
    assert_eq!(simulation.model().deliveries, 6);
}
