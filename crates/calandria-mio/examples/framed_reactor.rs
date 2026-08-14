//! Echoes one bounded frame through a concrete Mio-backed reactor duty.

use std::{
    error::Error,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream as StdTcpStream},
    num::NonZeroUsize,
    thread,
};

use calandria::{
    DedicatedHost, DedicatedOutcome, Duty, EmbeddedHost, HostConfig, Interest, Moment,
    MonotonicClock, Next, PollEvent, PollEvents, Readiness, ResourceOwnerId, ResourceTable,
    PollReport, ResourceToken, Span, Turn, WaitOutcome, WorkCount,
};
use calandria_mio::{MioError, MioPoller, MioPollerLimits};
use mio::net::TcpStream;

#[path = "framed_reactor/connection.rs"]
mod connection;

use connection::{Connection, MAX_FRAME};

const IO_BUDGET: usize = 8;

type BoxError = Box<dyn Error + Send + Sync>;

#[derive(Debug)]
struct FramedReactor {
    poller: MioPoller,
    events: PollEvents,
    resources: ResourceTable<u64, Connection>,
    token: ResourceToken,
    poll_saturated: bool,
    stale_backend_events: u64,
    stale_resource_events: u64,
    io_operations: u64,
}

impl FramedReactor {
    fn new(stream: TcpStream) -> Result<Self, BoxError> {
        let limits = MioPollerLimits::new(nonzero(32), nonzero(1));
        let mut poller = MioPoller::new(limits)?;
        let mut resources =
            ResourceTable::new(ResourceOwnerId::new(1), limits.registrations());
        let token = resources.admit(1, Connection::new(stream))?;
        let (_, connection) = resources.get_mut(token)?;
        poller.register(&mut connection.stream, token, Interest::READABLE)?;
        let events = poller.event_batch();

        Ok(Self {
            poller,
            events,
            resources,
            token,
            poll_saturated: false,
            stale_backend_events: 0,
            stale_resource_events: 0,
            io_operations: 0,
        })
    }

    fn wait_for_io(&mut self, maximum: Span) -> Result<WaitOutcome, MioError> {
        let report = self.poller.poll(maximum, &mut self.events)?;
        self.observe_poll(report);
        Ok(if report.observed() == 0 {
            WaitOutcome::Idle
        } else {
            WaitOutcome::Notified
        })
    }

    fn ingest_poll_batches(&mut self) -> Result<usize, MioError> {
        let mut work = self.ingest_readiness();
        if self.poll_saturated {
            let report = self.poller.poll(Span::ZERO, &mut self.events)?;
            self.observe_poll(report);
            work = work.saturating_add(self.ingest_readiness());
        }
        Ok(work)
    }

    fn observe_poll(&mut self, report: PollReport) {
        self.poll_saturated = report.saturated();
        self.stale_backend_events = self.stale_backend_events
            .saturating_add(to_u64(report.stale()));
    }

    fn ingest_readiness(&mut self) -> usize {
        let mut work = 0;
        for event in self.events.drain() {
            work += 1;
            let PollEvent::Resource { token, readiness } = event else {
                continue;
            };
            match self.resources.get_mut(token) {
                Ok((_, connection)) => connection.observe(readiness),
                Err(_) => {
                    self.stale_resource_events =
                        self.stale_resource_events.saturating_add(1);
                }
            }
        }
        work
    }

    fn progress_io(&mut self, mut budget: usize) -> Result<usize, BoxError> {
        let mut work = 0;
        let (_, connection) = self.resources.get_mut(self.token)?;

        while budget > 0 && connection.read_ready && connection.write.is_empty() {
            budget -= 1;
            work += 1;
            let mut chunk = [0_u8; 512];
            match connection.stream.read(&mut chunk) {
                Ok(0) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into()),
                Ok(read) => {
                    if connection.read.len().saturating_add(read) > MAX_FRAME + 4 {
                        return Err(io::Error::other("frame exceeds configured limit").into());
                    }
                    connection.read.extend_from_slice(&chunk[..read]);
                    if connection.prepare_echo()? {
                        connection.read_ready = false;
                    }
                }
                Err(source) if source.kind() == io::ErrorKind::WouldBlock => {
                    connection.read_ready = false;
                }
                Err(source) => return Err(source.into()),
            }
        }

        while budget > 0
            && connection.write_ready
            && connection.written < connection.write.len()
        {
            budget -= 1;
            work += 1;
            match connection.stream.write(&connection.write[connection.written..]) {
                Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero).into()),
                Ok(written) => connection.written += written,
                Err(source) if source.kind() == io::ErrorKind::WouldBlock => {
                    connection.write_ready = false;
                }
                Err(source) => return Err(source.into()),
            }
        }

        Ok(work)
    }

    fn sync_interest(&mut self) -> Result<(), BoxError> {
        let (poller, resources) = (&mut self.poller, &mut self.resources);
        let (_, connection) = resources.get_mut(self.token)?;
        let desired = connection.desired_interest();
        if desired != connection.interest {
            poller.reregister(&mut connection.stream, self.token, desired)?;
            connection.interest = desired;
        }
        Ok(())
    }

    fn finish_if_written(&mut self) -> Result<bool, BoxError> {
        let complete = {
            let (_, connection) = self.resources.get(self.token)?;
            !connection.write.is_empty() && connection.written == connection.write.len()
        };
        if !complete {
            return Ok(false);
        }

        let (poller, resources) = (&mut self.poller, &mut self.resources);
        let (_, connection) = resources.get_mut(self.token)?;
        poller.deregister(&mut connection.stream, self.token)?;
        let _owned = resources.remove(self.token)?;
        Ok(true)
    }
}

impl Duty for FramedReactor {
    type Error = BoxError;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        let mut work = self.ingest_poll_batches()?;
        work = work.saturating_add(self.progress_io(IO_BUDGET)?);
        self.sync_interest()?;
        if self.finish_if_written()? {
            self.io_operations = self.io_operations.saturating_add(to_u64(work));
            return Ok(Turn::stopped(WorkCount::new(to_u64(work))));
        }

        let local_work = {
            let (_, connection) = self.resources.get(self.token)?;
            connection.has_local_work()
        };
        self.io_operations = self.io_operations.saturating_add(to_u64(work));
        let next = if self.poll_saturated || local_work {
            Next::Now
        } else {
            Next::Wake
        };
        Ok(Turn::new(WorkCount::new(to_u64(work)), next))
    }
}

fn main() -> Result<(), BoxError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let client = thread::spawn(move || echo_client(address));
    let (server, _) = listener.accept()?;
    server.set_nonblocking(true)?;

    let reactor = FramedReactor::new(TcpStream::from_std(server))?;
    let host = EmbeddedHost::new(reactor, MonotonicClock::new(), HostConfig::default());
    let waiter = |reactor: &mut FramedReactor, maximum| reactor.wait_for_io(maximum);
    let dedicated = DedicatedHost::spawn("framed-reactor", host, waiter)?;
    let exit = dedicated
        .join()
        .map_err(|_| io::Error::other("framed reactor panicked"))?;
    if !matches!(exit.outcome(), DedicatedOutcome::Stopped) {
        return Err(io::Error::other("framed reactor did not stop cleanly").into());
    }
    let echoed = client
        .join()
        .map_err(|_| io::Error::other("framed client panicked"))??;

    println!(
        "echoed {echoed:?}; I/O: {}, stale backend/resource: {}/{}",
        exit.duty().io_operations, exit.duty().stale_backend_events,
        exit.duty().stale_resource_events,
    );
    Ok(())
}

fn echo_client(address: std::net::SocketAddr) -> io::Result<String> {
    let mut stream = StdTcpStream::connect(address)?;
    let payload = b"calandria";
    let length = u32::try_from(payload.len())
        .map_err(|_| io::Error::other("example payload length exceeds u32"))?;
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(payload)?;

    let mut echo = vec![0_u8; payload.len() + 4];
    stream.read_exact(&mut echo)?;
    String::from_utf8(echo[4..].to_vec())
        .map_err(|source| io::Error::new(io::ErrorKind::InvalidData, source))
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("example limit must be nonzero"))
}

fn to_u64(value: usize) -> u64 {
    u64::try_from(value)
        .unwrap_or_else(|_| panic!("example work count exceeds diagnostic domain"))
}
