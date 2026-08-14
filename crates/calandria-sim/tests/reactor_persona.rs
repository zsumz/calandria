//! Kafka-shaped one/many deterministic execution with one concrete duty type.

use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroUsize,
};

use calandria::{Duty, Moment, Next, Retained, RetainedBytes, Span, Turn, WorkCount};
use calandria_sim::{
    ActionContext, Delivery, DutyId, EntropySeed, EntropyStreamId, Model, Monitor, RunEnd,
    Scheduler, Seeded, Simulation, SimulationLimits, TimelineId, Topology, Trace, TraceLimits,
};

const INBOX_MESSAGES: usize = 4;
const INBOX_BYTES: u64 = 64;
const TURN_BUDGET: usize = 2;

#[derive(Debug)]
enum BrokerCommand {
    Produce { operation: u64, value: Vec<u8> },
    Shutdown,
}

impl Retained for BrokerCommand {
    fn retained_bytes(&self) -> RetainedBytes {
        match self {
            Self::Produce { value, .. } => RetainedBytes::new(
                u64::try_from(value.capacity())
                    .unwrap_or_else(|_| panic!("persona payload capacity exceeds u64")),
            ),
            Self::Shutdown => RetainedBytes::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BrokerFailure {
    MessageCapacity,
    ByteCapacity,
}

#[derive(Debug)]
struct BrokerShard {
    id: DutyId,
    inbox: VecDeque<BrokerCommand>,
    inbox_bytes: RetainedBytes,
    operations: u64,
    bytes: u64,
    stopping: bool,
}

impl BrokerShard {
    fn new(id: DutyId) -> Self {
        Self {
            id,
            inbox: VecDeque::with_capacity(INBOX_MESSAGES),
            inbox_bytes: RetainedBytes::ZERO,
            operations: 0,
            bytes: 0,
            stopping: false,
        }
    }

    fn accept(&mut self, command: BrokerCommand) -> Result<(), BrokerFailure> {
        if self.inbox.len() >= INBOX_MESSAGES {
            return Err(BrokerFailure::MessageCapacity);
        }
        let retained = command.retained_bytes();
        let Some(next) = self.inbox_bytes.checked_add(retained) else {
            return Err(BrokerFailure::ByteCapacity);
        };
        if next.get() > INBOX_BYTES {
            return Err(BrokerFailure::ByteCapacity);
        }
        self.inbox.push_back(command);
        self.inbox_bytes = next;
        Ok(())
    }
}

impl Duty for BrokerShard {
    type Error = BrokerFailure;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        let mut work = 0usize;
        while work < TURN_BUDGET {
            let Some(command) = self.inbox.pop_front() else {
                break;
            };
            self.inbox_bytes = self
                .inbox_bytes
                .checked_sub(command.retained_bytes())
                .unwrap_or_else(|| panic!("persona inbox accounting diverged"));
            match command {
                BrokerCommand::Produce { operation, value } => {
                    self.operations = self.operations.saturating_add(operation);
                    self.bytes = self.bytes.saturating_add(
                        u64::try_from(value.len())
                            .unwrap_or_else(|_| panic!("persona payload length exceeds u64")),
                    );
                }
                BrokerCommand::Shutdown => self.stopping = true,
            }
            work += 1;
        }
        let work = WorkCount::new(
            u64::try_from(work).unwrap_or_else(|_| panic!("persona work exceeds u64")),
        );
        Ok(if self.stopping && self.inbox.is_empty() {
            Turn::stopped(work)
        } else if self.inbox.is_empty() {
            Turn::new(work, Next::Wake)
        } else {
            Turn::runnable(work)
        })
    }
}

#[derive(Debug)]
struct BrokerWorld {
    shards: BTreeMap<DutyId, BrokerShard>,
}

impl BrokerWorld {
    fn new(duties: &[DutyId]) -> Self {
        Self {
            shards: duties
                .iter()
                .copied()
                .map(|id| (id, BrokerShard::new(id)))
                .collect(),
        }
    }

    fn shard(&mut self, duty: DutyId) -> &mut BrokerShard {
        self.shards
            .get_mut(&duty)
            .unwrap_or_else(|| panic!("persona duty must belong to topology"))
    }
}

impl Model for BrokerWorld {
    type Event = BrokerCommand;
    type Observation = ();
    type Error = BrokerFailure;

    fn turn(
        &mut self,
        duty: DutyId,
        now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        self.shard(duty).turn(now)
    }

    fn deliver(
        &mut self,
        duty: DutyId,
        delivery: Delivery<Self::Event>,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        let shard = self.shard(duty);
        shard.accept(delivery.into_event())?;
        shard.turn(context.now())
    }
}

#[test]
fn one_and_many_topologies_share_the_same_bounded_broker_duty() {
    exercise([DutyId::new(0)]);
    exercise([DutyId::new(0), DutyId::new(1), DutyId::new(2)]);
}

fn exercise<const N: usize>(duties: [DutyId; N]) {
    let mut original = Simulation::with_scheduler(
        TimelineId::new(71),
        BrokerWorld::new(&duties),
        topology(duties),
        SimulationLimits::default(),
        Seeded::new(EntropySeed::new(73), EntropyStreamId::new(1)),
    )
    .unwrap_or_else(|error| panic!("persona simulation must build: {error}"))
    .with_monitor(Trace::new(trace_limits()));
    inject(&mut original, &duties);
    let report = original
        .run_to_completion()
        .unwrap_or_else(|error| panic!("persona simulation must complete: {error:?}"));
    assert_eq!(report.end(), RunEnd::Completed);
    assert_shards(original.model(), &duties);

    let replay = original.monitor().replay();
    let mut repeated = Simulation::with_scheduler(
        TimelineId::new(71),
        BrokerWorld::new(&duties),
        topology(duties),
        SimulationLimits::default(),
        replay,
    )
    .unwrap_or_else(|error| panic!("persona replay must build: {error}"));
    inject(&mut repeated, &duties);
    repeated
        .run_to_completion()
        .unwrap_or_else(|error| panic!("persona replay must complete: {error:?}"));
    assert!(repeated.scheduler().is_complete());
    assert_shards(repeated.model(), &duties);
}

fn inject<S, N>(simulation: &mut Simulation<BrokerWorld, S, N>, duties: &[DutyId])
where
    S: Scheduler,
    N: Monitor<BrokerWorld>,
{
    for duty in duties {
        for (operation, value) in [(1, b"alpha".to_vec()), (2, b"beta".to_vec())] {
            let _ = simulation
                .inject(*duty, BrokerCommand::Produce { operation, value })
                .unwrap_or_else(|error| panic!("persona produce must fit: {error}"));
        }
        let _ = simulation
            .inject_after(*duty, Span::from_nanos(1), BrokerCommand::Shutdown)
            .unwrap_or_else(|error| panic!("persona shutdown must fit: {error}"));
    }
}

fn assert_shards(world: &BrokerWorld, duties: &[DutyId]) {
    assert_eq!(world.shards.len(), duties.len());
    for (id, shard) in &world.shards {
        assert_eq!(shard.id, *id);
        assert_eq!(shard.operations, 3);
        assert_eq!(shard.bytes, 9);
        assert!(shard.stopping);
        assert!(shard.inbox.is_empty());
    }
}

fn topology<const N: usize>(duties: [DutyId; N]) -> Topology {
    Topology::new(duties).unwrap_or_else(|error| panic!("persona topology must be valid: {error}"))
}

fn trace_limits() -> TraceLimits {
    TraceLimits::new(
        NonZeroUsize::new(32).unwrap_or_else(|| panic!("trace count must be nonzero")),
        RetainedBytes::ZERO,
    )
}
