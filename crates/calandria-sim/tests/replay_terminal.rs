//! Exact replay behavior at clean completion and trace exhaustion.

use core::{convert::Infallible, num::NonZeroUsize};

use calandria::{Moment, RetainedBytes, Turn, WorkCount};
use calandria_sim::{
    ActionContext, Delivery, DutyId, Model, Replay, ReplayDivergence, Simulation, SimulationLimits,
    StepError, TimelineId, Topology, Trace, TraceLimits,
};

#[derive(Debug)]
struct StoppingWorld;

impl Model for StoppingWorld {
    type Event = ();
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        _duty: DutyId,
        _now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
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

#[test]
fn replay_rejects_clean_completion_with_trace_remaining() {
    let replay = trace_stopping_world(topology([1, 2]));
    let mut shorter = replay_stopping_world(topology([1]), replay);
    first_action(&mut shorter);
    let Err(error) = shorter.step() else {
        panic!("early clean completion must diverge");
    };
    assert_eq!(
        error.to_string(),
        "scheduler failed: completion at 1 left 1 trace entries"
    );

    assert!(matches!(
        error,
        StepError::Scheduler(ReplayDivergence::TraceRemaining {
            position,
            remaining: 1,
        }) if position.get() == 1
    ));
}

#[test]
fn replay_rejects_ready_work_after_trace_exhaustion() {
    let replay = trace_stopping_world(topology([1]));
    let mut longer = replay_stopping_world(topology([1, 2]), replay);
    first_action(&mut longer);
    let Err(error) = longer.step() else {
        panic!("work after the trace must diverge");
    };
    assert_eq!(
        error.to_string(),
        "scheduler failed: trace ended at 1, but 1 actions were ready at 0ns"
    );

    assert!(matches!(
        error,
        StepError::Scheduler(ReplayDivergence::TraceExhausted {
            position,
            ready: 1,
            ..
        }) if position.get() == 1
    ));
}

fn first_action(simulation: &mut Simulation<StoppingWorld, Replay>) {
    simulation
        .step()
        .unwrap_or_else(|error| panic!("first traced action must match: {error}"));
}

fn trace_stopping_world(topology: Topology) -> Replay {
    let mut traced = Simulation::new(
        TimelineId::new(64),
        StoppingWorld,
        topology,
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("trace source must build: {error}"))
    .with_monitor(Trace::new(TraceLimits::new(
        NonZeroUsize::new(4).unwrap_or_else(|| panic!("trace limit must be nonzero")),
        RetainedBytes::ZERO,
    )));
    traced
        .run_to_completion()
        .unwrap_or_else(|error| panic!("trace source must complete: {error}"));
    traced.monitor().replay()
}

fn replay_stopping_world(topology: Topology, replay: Replay) -> Simulation<StoppingWorld, Replay> {
    Simulation::with_scheduler(
        TimelineId::new(64),
        StoppingWorld,
        topology,
        SimulationLimits::default(),
        replay,
    )
    .unwrap_or_else(|error| panic!("replay must build: {error}"))
}

fn topology<const N: usize>(duties: [u32; N]) -> Topology {
    Topology::new(duties.map(DutyId::new))
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}
