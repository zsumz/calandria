//! Runs one production `Duty` under deterministic virtual time.
use core::convert::Infallible;

use calandria::{Deadline, Duty, Moment, Turn, WorkCount};
use calandria_sim::{
    ActionContext, Delivery, DutyId, Model, RunEnd, Simulation, SimulationLimits, TimelineId,
    Topology,
};

const MAINTENANCE: DutyId = DutyId::new(7);

#[derive(Debug)]
struct Maintenance {
    turns: u8,
}

impl Duty for Maintenance {
    type Error = Infallible;

    fn turn(&mut self, now: Moment) -> Result<Turn, Self::Error> {
        self.turns = self.turns.saturating_add(1);
        if now < Moment::from_nanos(10) {
            return Ok(Turn::until(
                WorkCount::new(1),
                Deadline::at(Moment::from_nanos(10)),
            ));
        }
        Ok(Turn::stopped(WorkCount::new(1)))
    }
}

#[derive(Debug)]
struct World {
    maintenance: Maintenance,
}

impl Model for World {
    type Event = ();
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        duty: DutyId,
        now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        assert_eq!(duty, MAINTENANCE);
        self.maintenance.turn(now)
    }

    fn deliver(
        &mut self,
        _duty: DutyId,
        _delivery: Delivery<Self::Event>,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        Ok(Turn::waiting())
    }
}

fn main() {
    let topology = Topology::new([MAINTENANCE])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"));
    let mut simulation = Simulation::new(
        TimelineId::new(1),
        World {
            maintenance: Maintenance { turns: 0 },
        },
        topology,
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));

    let report = simulation
        .run_to_completion()
        .unwrap_or_else(|error| panic!("maintenance must complete: {error}"));
    assert_eq!(report.end(), RunEnd::Completed);
    assert_eq!(report.snapshot().now(), Moment::from_nanos(10));
    assert_eq!(simulation.model().maintenance.turns, 2);
}
