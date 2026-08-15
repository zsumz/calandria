//! Scheduler, monitor, and delivery commit-boundary lifecycle tests.

use core::{convert::Infallible, fmt};
use std::panic::{AssertUnwindSafe, catch_unwind};

use calandria::{Deadline, Moment, Turn, WorkCount};
use calandria_sim::{
    ActionContext, ActionKey, ActionMeta, ActionRecord, Delivery, DutyId, KernelFailure, Model,
    Monitor, ReadySet, Scheduler, Simulation, SimulationLimits, SimulationPhase, SimulationView,
    Step, StepError, TimelineId, Topology,
};

#[derive(Clone, Copy, Debug)]
enum Behavior {
    Stop,
    Deadline,
    DeliveryStopsWithPending,
}

#[derive(Clone, Copy, Debug)]
struct World {
    behavior: Behavior,
    turns: u64,
    deliveries: u64,
}

impl World {
    const fn new(behavior: Behavior) -> Self {
        Self {
            behavior,
            turns: 0,
            deliveries: 0,
        }
    }
}

impl Model for World {
    type Event = ();
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        _duty: DutyId,
        _now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        self.turns += 1;
        Ok(match self.behavior {
            Behavior::Stop => Turn::stopped(WorkCount::new(1)),
            Behavior::Deadline => {
                Turn::until(WorkCount::new(1), Deadline::at(Moment::from_nanos(10)))
            }
            Behavior::DeliveryStopsWithPending => Turn::waiting(),
        })
    }

    fn deliver(
        &mut self,
        duty: DutyId,
        _delivery: Delivery<Self::Event>,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        self.deliveries += 1;
        if matches!(self.behavior, Behavior::DeliveryStopsWithPending) {
            context
                .send(duty, ())
                .unwrap_or_else(|error| panic!("self-delivery must fit: {error}"));
            return Ok(Turn::stopped(WorkCount::new(1)));
        }
        Ok(Turn::waiting())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlannedFailure;

impl fmt::Display for PlannedFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("planned scheduler failure")
    }
}

impl core::error::Error for PlannedFailure {}

#[derive(Clone, Copy, Debug)]
enum Hook {
    PanicChoose,
    PanicCommitted,
    PanicFinished,
    FailFinished,
}

#[derive(Debug)]
struct HookScheduler(Hook);

impl Scheduler for HookScheduler {
    type Error = PlannedFailure;

    fn choose(&mut self, _now: Moment, ready: ReadySet<'_>) -> Result<ActionKey, Self::Error> {
        assert!(!ready.is_empty());
        assert_eq!(ready.actions().len(), ready.len());
        if matches!(self.0, Hook::PanicChoose) {
            panic!("planned choose panic");
        }
        Ok(ready
            .first()
            .unwrap_or_else(|| panic!("ready set must be nonempty")))
    }

    fn committed(&mut self, _action: ActionMeta, _turn: Turn) -> Result<(), Self::Error> {
        if matches!(self.0, Hook::PanicCommitted) {
            panic!("planned committed panic");
        }
        Ok(())
    }

    fn finished(&mut self) -> Result<(), Self::Error> {
        match self.0 {
            Hook::PanicFinished => panic!("planned finished panic"),
            Hook::FailFinished => Err(PlannedFailure),
            Hook::PanicChoose | Hook::PanicCommitted => Ok(()),
        }
    }
}

#[test]
fn scheduler_panics_poison_selection_commit_and_completion_boundaries() {
    let mut choose = scheduled(Hook::PanicChoose);
    assert!(catch_unwind(AssertUnwindSafe(|| choose.step())).is_err());
    assert!(choose.is_poisoned());
    assert_eq!(choose.snapshot().actions(), 0);
    assert!(matches!(choose.step(), Err(StepError::Poisoned)));

    let mut committed = scheduled(Hook::PanicCommitted);
    assert!(catch_unwind(AssertUnwindSafe(|| committed.step())).is_err());
    assert!(committed.is_poisoned());
    assert_eq!(committed.snapshot().actions(), 1);
    assert_eq!(committed.snapshot().stopped(), 1);
    assert_eq!(committed.phase(), SimulationPhase::Active);

    let mut finished = scheduled(Hook::PanicFinished);
    assert!(matches!(finished.step(), Ok(Step::Action(_))));
    assert!(catch_unwind(AssertUnwindSafe(|| finished.step())).is_err());
    assert!(finished.is_poisoned());
    assert_eq!(finished.phase(), SimulationPhase::Active);
}

#[test]
fn scheduler_completion_failure_is_terminal_after_domain_commit() {
    let mut simulation = scheduled(Hook::FailFinished);
    assert!(matches!(simulation.step(), Ok(Step::Action(_))));
    let Err(error) = simulation.step() else {
        panic!("scheduler must validate clean completion");
    };
    assert!(matches!(error, StepError::Scheduler(PlannedFailure)));
    assert_eq!(simulation.phase(), SimulationPhase::Failed);
    assert_eq!(simulation.snapshot().stopped(), 1);
}

#[derive(Debug, Default)]
struct InspectMonitor {
    calls: u64,
}

impl Monitor<World> for InspectMonitor {
    type Error = Infallible;

    fn after_action(
        &mut self,
        view: SimulationView<'_, World>,
        action: &ActionRecord<()>,
    ) -> Result<(), Self::Error> {
        assert_eq!(view.model().turns, 1);
        assert_eq!(view.topology().duties(), [DutyId::new(1)]);
        assert_eq!(
            view.duty(DutyId::new(1))
                .map(calandria_sim::DutySnapshot::turns),
            Some(1)
        );
        assert!(view.duty(DutyId::new(9)).is_none());
        assert_eq!(view.snapshot().actions(), 1);
        assert_eq!(view.snapshot().actions_at_moment(), 1);
        assert_eq!(
            view.snapshot().retained_bytes(),
            calandria::RetainedBytes::ZERO
        );
        assert_eq!(action.meta().id().get(), 0);
        self.calls += 1;
        Ok(())
    }
}

#[derive(Debug)]
struct PanicMonitor;

impl Monitor<World> for PanicMonitor {
    type Error = Infallible;

    fn after_action(
        &mut self,
        _view: SimulationView<'_, World>,
        _action: &ActionRecord<()>,
    ) -> Result<(), Self::Error> {
        panic!("planned monitor panic");
    }
}

#[test]
fn monitors_observe_immutable_owner_state_and_panics_poison_after_commit() {
    let simulation = Simulation::new(
        TimelineId::new(101),
        World::new(Behavior::Stop),
        topology(),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));
    let mut inspected = simulation.with_monitor(InspectMonitor::default());
    assert_eq!(inspected.topology().duties(), [DutyId::new(1)]);
    assert!(matches!(inspected.step(), Ok(Step::Action(_))));
    assert_eq!(inspected.monitor().calls, 1);

    let simulation = Simulation::new(
        TimelineId::new(102),
        World::new(Behavior::Stop),
        topology(),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));
    let mut panicking = simulation.with_monitor(PanicMonitor);
    assert!(catch_unwind(AssertUnwindSafe(|| panicking.step())).is_err());
    assert!(panicking.is_poisoned());
    assert_eq!(panicking.snapshot().actions(), 1);
    assert_eq!(panicking.snapshot().stopped(), 1);

    let model = Simulation::new(
        TimelineId::new(105),
        World::new(Behavior::Stop),
        topology(),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"))
    .into_model();
    assert_eq!(model.turns, 0);
}

#[test]
fn time_advances_to_the_earliest_event_before_a_later_owner_deadline() {
    let mut simulation = Simulation::new(
        TimelineId::new(103),
        World::new(Behavior::Deadline),
        topology(),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));
    simulation
        .inject_at(DutyId::new(1), Moment::from_nanos(5), ())
        .unwrap_or_else(|error| panic!("future event must fit: {error}"));

    assert!(matches!(simulation.step(), Ok(Step::Action(_))));
    assert!(matches!(
        simulation.step(),
        Ok(Step::TimeAdvanced { from, to })
            if from == Moment::ORIGIN && to == Moment::from_nanos(5)
    ));
}

#[test]
fn delivery_cannot_stop_while_retaining_a_new_self_delivery() {
    let mut simulation = Simulation::new(
        TimelineId::new(104),
        World::new(Behavior::DeliveryStopsWithPending),
        topology(),
        SimulationLimits::default(),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"));
    simulation
        .inject(DutyId::new(1), ())
        .unwrap_or_else(|error| panic!("immediate event must fit: {error}"));

    let Err(error) = simulation.step() else {
        panic!("stopping with a staged self-delivery must fail");
    };
    assert!(matches!(
        error,
        StepError::Kernel(KernelFailure::StoppedWithPending { duty, pending })
            if duty == DutyId::new(1) && pending == 1
    ));
    assert_eq!(simulation.phase(), SimulationPhase::Failed);
    assert_eq!(simulation.snapshot().pending_events(), 0);
    assert_eq!(simulation.model().deliveries, 1);
}

fn scheduled(hook: Hook) -> Simulation<World, HookScheduler> {
    Simulation::with_scheduler(
        TimelineId::new(100),
        World::new(Behavior::Stop),
        topology(),
        SimulationLimits::default(),
        HookScheduler(hook),
    )
    .unwrap_or_else(|error| panic!("simulation must build: {error}"))
}

fn topology() -> Topology {
    Topology::new([DutyId::new(1)])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}
