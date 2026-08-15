//! Bounded structured-observation commit, rejection, and rollback tests.

use core::{convert::Infallible, fmt, num::NonZeroUsize};

use calandria::{Moment, Next, Retained, RetainedBytes, Turn, WorkCount};
use calandria_sim::{
    ActionContext, ActionRecord, Delivery, DutyId, Model, Monitor, ObservationError,
    ObservationFailure, Simulation, SimulationLimits, SimulationPhase, SimulationView, Step,
    StepError, TimelineId, Topology,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Observation {
    id: u8,
    bytes: RetainedBytes,
}

impl Retained for Observation {
    fn retained_bytes(&self) -> RetainedBytes {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Commit,
    CountCapacity,
    ByteCapacity,
    ByteOverflow,
    Rollback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlannedFailure;

impl fmt::Display for PlannedFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("planned observation rollback")
    }
}

impl core::error::Error for PlannedFailure {}

#[derive(Debug)]
struct ObservationWorld {
    scenario: Scenario,
    rejected: Option<(Observation, ObservationFailure)>,
}

impl ObservationWorld {
    const fn new(scenario: Scenario) -> Self {
        Self {
            scenario,
            rejected: None,
        }
    }

    fn retain_rejection(&mut self, error: ObservationError<Observation>) {
        assert!(!error.to_string().is_empty());
        let failure = error.failure();
        self.rejected = Some((error.into_observation(), failure));
    }
}

impl Model for ObservationWorld {
    type Event = ();
    type Observation = Observation;
    type Error = PlannedFailure;

    fn turn(
        &mut self,
        _duty: DutyId,
        _now: Moment,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        match self.scenario {
            Scenario::Commit => {
                observe(context, observation(1, 2));
                observe(context, observation(2, 3));
            }
            Scenario::CountCapacity => {
                observe(context, observation(1, 0));
                let error = rejection(
                    context.observe(observation(2, 0)),
                    "second observation must exceed count",
                );
                self.retain_rejection(error);
            }
            Scenario::ByteCapacity => {
                observe(context, observation(1, 2));
                let error = rejection(
                    context.observe(observation(2, 2)),
                    "second observation must exceed bytes",
                );
                self.retain_rejection(error);
            }
            Scenario::ByteOverflow => {
                observe(context, observation(1, 1));
                let error = rejection(
                    context.observe(observation(2, u64::MAX)),
                    "observation accounting must not wrap",
                );
                self.retain_rejection(error);
            }
            Scenario::Rollback => {
                observe(context, observation(1, 1));
                return Err(PlannedFailure);
            }
        }
        Ok(Turn::stopped(WorkCount::new(1)))
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

#[derive(Debug, Default)]
struct Collect {
    actions: u64,
    observations: Vec<Observation>,
}

impl Monitor<ObservationWorld> for Collect {
    type Error = Infallible;

    fn after_action(
        &mut self,
        _view: SimulationView<'_, ObservationWorld>,
        action: &ActionRecord<Observation>,
    ) -> Result<(), Self::Error> {
        self.actions += 1;
        self.observations.extend_from_slice(action.observations());
        Ok(())
    }
}

#[test]
fn committed_observations_reach_action_and_monitor_in_emission_order() {
    let limits = SimulationLimits::default()
        .with_observations_per_action(nonzero(2))
        .with_observation_bytes_per_action(RetainedBytes::new(5));
    assert_eq!(limits.observations_per_action(), nonzero(2));
    assert_eq!(limits.observation_bytes_per_action(), RetainedBytes::new(5));
    let mut simulation = simulation(Scenario::Commit, limits);

    let Step::Action(record) = simulation
        .step()
        .unwrap_or_else(|error| panic!("observation action must commit: {error}"))
    else {
        panic!("initial step must execute an action");
    };
    assert_eq!(
        record.into_observations(),
        [observation(1, 2), observation(2, 3)]
    );
    assert_eq!(simulation.monitor().actions, 1);
    assert_eq!(
        simulation.monitor().observations,
        [observation(1, 2), observation(2, 3)]
    );
    let duty = simulation
        .duty(DutyId::new(1))
        .unwrap_or_else(|| panic!("topology duty must have a snapshot"));
    assert_eq!(duty.duty(), DutyId::new(1));
    assert_eq!(duty.next(), Next::Stop);
    assert_eq!(duty.turns(), 1);
    assert_eq!(duty.deliveries(), 0);
}

#[test]
fn observation_rejections_preserve_exact_ownership_and_reason() {
    assert_rejection(
        Scenario::CountCapacity,
        SimulationLimits::default().with_observations_per_action(nonzero(1)),
        observation(2, 0),
        ObservationFailure::Capacity { limit: nonzero(1) },
    );
    assert_rejection(
        Scenario::ByteCapacity,
        SimulationLimits::default()
            .with_observations_per_action(nonzero(2))
            .with_observation_bytes_per_action(RetainedBytes::new(3)),
        observation(2, 2),
        ObservationFailure::RetainedByteCapacity {
            limit: RetainedBytes::new(3),
            current: RetainedBytes::new(2),
            observation: RetainedBytes::new(2),
        },
    );
    assert_rejection(
        Scenario::ByteOverflow,
        SimulationLimits::default()
            .with_observations_per_action(nonzero(2))
            .with_observation_bytes_per_action(RetainedBytes::new(u64::MAX)),
        observation(2, u64::MAX),
        ObservationFailure::RetainedByteOverflow {
            current: RetainedBytes::new(1),
            observation: RetainedBytes::new(u64::MAX),
        },
    );
}

#[test]
fn observations_are_not_published_when_model_action_rolls_back() {
    let mut simulation = simulation(Scenario::Rollback, SimulationLimits::default());
    let Err(error) = simulation.step() else {
        panic!("planned model failure must roll back");
    };

    assert!(matches!(error, StepError::Model { .. }));
    assert_eq!(simulation.phase(), SimulationPhase::Failed);
    assert_eq!(simulation.monitor().actions, 0);
    assert!(simulation.monitor().observations.is_empty());
}

fn assert_rejection(
    scenario: Scenario,
    limits: SimulationLimits,
    observation: Observation,
    failure: ObservationFailure,
) {
    let mut simulation = simulation(scenario, limits);
    simulation
        .step()
        .unwrap_or_else(|error| panic!("rejected observation must not fail its action: {error}"));
    assert_eq!(simulation.model().rejected, Some((observation, failure)));
    assert_eq!(simulation.monitor().actions, 1);
    assert_eq!(simulation.monitor().observations.len(), 1);
}

fn simulation(
    scenario: Scenario,
    limits: SimulationLimits,
) -> Simulation<ObservationWorld, calandria_sim::Fifo, Collect> {
    Simulation::new(
        TimelineId::new(81),
        ObservationWorld::new(scenario),
        Topology::new([DutyId::new(1)])
            .unwrap_or_else(|error| panic!("topology must be valid: {error}")),
        limits,
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"))
    .with_monitor(Collect::default())
}

fn observe(context: &mut ActionContext<'_, (), Observation>, value: Observation) {
    context
        .observe(value)
        .unwrap_or_else(|error| panic!("observation must fit: {error}"));
}

fn rejection(
    result: Result<(), ObservationError<Observation>>,
    message: &str,
) -> ObservationError<Observation> {
    match result {
        Ok(()) => panic!("{message}"),
        Err(error) => error,
    }
}

const fn observation(id: u8, bytes: u64) -> Observation {
    Observation {
        id,
        bytes: RetainedBytes::new(bytes),
    }
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
