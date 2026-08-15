//! Selector registration, wake, fencing, and readiness translation contracts.

use std::{
    error::Error,
    io,
    net::{SocketAddr, TcpStream as StdTcpStream},
    num::NonZeroUsize,
};

use calandria::{
    Interest, PollEvent, PollEvents, Poller, ResourceGeneration, ResourceOwnerId, ResourceSlotId,
    ResourceToken, Span,
};
use calandria_mio::{MioError, MioPoller, MioPollerLimits};
use mio::{Interest as MioInterest, Registry, Token, event::Source, net::TcpListener};

#[path = "poller_support/mod.rs"]
mod support;

use support::supported_compound_interest;

#[test]
fn explicit_wake_releases_the_selector() -> Result<(), Box<dyn Error>> {
    let limits = MioPollerLimits::new(nonzero(8), nonzero(8));
    let mut poller = MioPoller::new(limits)?;
    let wake = poller.wake_handle();
    let mut events = poller.event_batch();

    wake.wake()?;
    let report = poller.poll(Span::from_nanos(1_000_000_000), &mut events)?;

    assert!(report.observed() >= 1);
    assert_eq!(report.wakes(), 1);
    assert!(events.iter().any(|event| *event == PollEvent::Wake));
    Ok(())
}

#[test]
fn readiness_maps_back_to_the_exact_resource_generation() -> Result<(), Box<dyn Error>> {
    let limits = MioPollerLimits::new(nonzero(8), nonzero(2));
    let mut poller = MioPoller::new(limits)?;
    let mut listener = TcpListener::bind("127.0.0.1:0".parse()?)?;
    let address = listener.local_addr()?;
    let token = resource(3);
    poller.register(&mut listener, token, Interest::READABLE)?;
    let _client = StdTcpStream::connect(address)?;
    let mut events = poller.event_batch();

    let report = poller.poll(Span::from_nanos(1_000_000_000), &mut events)?;

    assert!(report.delivered() >= 1);
    assert!(events.iter().any(|event| matches!(
        event,
        PollEvent::Resource { token: observed, readiness }
            if *observed == token && readiness.is_readable()
    )));
    Ok(())
}

#[test]
fn unsupported_interest_is_rejected_before_identity_consumption() -> Result<(), Box<dyn Error>> {
    let limits = MioPollerLimits::new(nonzero(4), nonzero(1));
    let mut poller = MioPoller::new(limits)?;
    let mut listener = TcpListener::bind("127.0.0.1:0".parse()?)?;
    let interest = unsupported_interest();

    let error = match poller.register(&mut listener, resource(0), interest) {
        Ok(()) => panic!("unsupported interest must be rejected"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        MioError::UnsupportedInterest { interest: rejected } if rejected == interest
    ));
    assert_eq!(poller.snapshot().registrations(), 0);
    assert_eq!(poller.snapshot().backend_tokens_issued(), 0);
    #[cfg(not(target_os = "freebsd"))]
    assert!(matches!(
        poller.register(&mut listener, resource(0), Interest::LIO),
        Err(MioError::UnsupportedInterest { interest }) if interest == Interest::LIO
    ));
    Ok(())
}

#[test]
fn failed_backend_registration_consumes_its_backend_identity() -> Result<(), Box<dyn Error>> {
    let limits = MioPollerLimits::new(nonzero(4), nonzero(1));
    let mut poller = MioPoller::new(limits)?;
    let mut source = RejectingSource;

    let error = match poller.register(&mut source, resource(0), supported_compound_interest()) {
        Ok(()) => panic!("backend registration must fail"),
        Err(error) => error,
    };

    assert!(matches!(error, MioError::Io(_)));
    assert_eq!(poller.snapshot().registrations(), 0);
    assert_eq!(poller.snapshot().backend_tokens_issued(), 1);
    Ok(())
}

#[test]
fn failed_lifecycle_changes_preserve_registration_translation() -> Result<(), Box<dyn Error>> {
    let limits = MioPollerLimits::new(nonzero(4), nonzero(1));
    let mut poller = MioPoller::new(limits)?;
    let mut source = RejectLaterSource;
    let token = resource(0);
    poller.register(&mut source, token, Interest::READABLE)?;

    assert!(matches!(
        poller.reregister(&mut source, token, Interest::WRITABLE),
        Err(MioError::Io(_))
    ));
    assert_eq!(poller.snapshot().registrations(), 1);

    assert!(matches!(
        poller.deregister(&mut source, token),
        Err(MioError::Io(_))
    ));
    assert_eq!(poller.snapshot().registrations(), 1);
    Ok(())
}

#[test]
fn zero_wait_returns_an_empty_bounded_batch() -> Result<(), Box<dyn Error>> {
    let limits = MioPollerLimits::new(nonzero(4), nonzero(4));
    let mut poller = MioPoller::new(limits)?;
    let mut events = poller.event_batch();

    let report = Poller::poll(&mut poller, Span::ZERO, &mut events)?;

    assert_eq!(report.delivered(), 0);
    assert!(events.is_empty());
    Ok(())
}

#[test]
fn destination_capacity_is_validated_before_polling() -> Result<(), Box<dyn Error>> {
    let limits = MioPollerLimits::new(nonzero(4), nonzero(4));
    let mut poller = MioPoller::new(limits)?;
    let mut events = PollEvents::new(nonzero(2));
    events
        .try_push(PollEvent::Wake)
        .unwrap_or_else(|error| panic!("test event must fit: {error}"));

    let Err(error) = poller.poll(Span::ZERO, &mut events) else {
        panic!("undersized destination must be rejected");
    };

    assert!(matches!(
        error,
        MioError::DestinationTooSmall { required, actual }
            if required == nonzero(4) && actual == nonzero(2)
    ));
    assert!(events.is_empty());
    Ok(())
}

#[test]
fn configured_limits_are_stable_and_observable() -> Result<(), Box<dyn Error>> {
    let limits = MioPollerLimits::new(nonzero(7), nonzero(9));
    let poller = MioPoller::new(limits)?;

    assert_eq!(poller.limits(), limits);
    assert_eq!(poller.snapshot().limits(), limits);
    assert!(!poller.snapshot().token_space_exhausted());
    assert_eq!(poller.event_batch().capacity(), limits.events());
    Ok(())
}

#[test]
fn registration_capacity_is_enforced_before_selector_mutation() -> Result<(), Box<dyn Error>> {
    let limits = MioPollerLimits::new(nonzero(4), nonzero(1));
    let mut poller = MioPoller::new(limits)?;
    let address: SocketAddr = "127.0.0.1:0".parse()?;
    let mut first = TcpListener::bind(address)?;
    let mut second = TcpListener::bind(address)?;
    poller.register(&mut first, resource(0), Interest::READABLE)?;

    let error = match poller.register(&mut second, resource(1), Interest::READABLE) {
        Ok(()) => panic!("registration capacity must be enforced"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        MioError::RegistrationCapacity { limit } if limit == nonzero(1)
    ));
    assert_eq!(poller.snapshot().registrations(), 1);
    Ok(())
}

#[test]
fn exact_resource_generations_own_registration() -> Result<(), Box<dyn Error>> {
    let limits = MioPollerLimits::new(nonzero(4), nonzero(2));
    let mut poller = MioPoller::new(limits)?;
    let address: SocketAddr = "127.0.0.1:0".parse()?;
    let mut listener = TcpListener::bind(address)?;
    let token = resource(0);

    poller.register(&mut listener, token, Interest::READABLE)?;
    assert_eq!(poller.snapshot().registrations(), 1);
    assert!(matches!(
        poller.register(&mut listener, token, Interest::READABLE),
        Err(MioError::AlreadyRegistered { token: duplicate }) if duplicate == token
    ));

    poller.deregister(&mut listener, token)?;
    assert_eq!(poller.snapshot().registrations(), 0);
    assert!(matches!(
        poller.reregister(&mut listener, token, Interest::READABLE),
        Err(MioError::NotRegistered { token: missing }) if missing == token
    ));
    Ok(())
}

#[test]
fn wake_handles_are_independent_acknowledgement_domains() -> Result<(), Box<dyn Error>> {
    let poller = MioPoller::new(MioPollerLimits::new(nonzero(4), nonzero(4)))?;
    let first = poller.wake_handle();
    let second = poller.wake_handle();

    first.wake()?;
    second.wake()?;
    first.acknowledge();

    assert!(!first.is_requested());
    assert!(second.is_requested());
    second.acknowledge();
    Ok(())
}

#[derive(Debug)]
struct RejectLaterSource;

impl Source for RejectLaterSource {
    fn register(
        &mut self,
        _registry: &Registry,
        _token: Token,
        _interests: MioInterest,
    ) -> io::Result<()> {
        Ok(())
    }

    fn reregister(
        &mut self,
        _registry: &Registry,
        _token: Token,
        _interests: MioInterest,
    ) -> io::Result<()> {
        Err(io::Error::other("planned reregistration rejection"))
    }

    fn deregister(&mut self, _registry: &Registry) -> io::Result<()> {
        Err(io::Error::other("planned deregistration rejection"))
    }
}

#[derive(Debug)]
struct RejectingSource;

impl Source for RejectingSource {
    fn register(
        &mut self,
        _registry: &Registry,
        _token: Token,
        _interests: MioInterest,
    ) -> io::Result<()> {
        Err(io::Error::other("planned registration rejection"))
    }

    fn reregister(
        &mut self,
        _registry: &Registry,
        _token: Token,
        _interests: MioInterest,
    ) -> io::Result<()> {
        Err(io::Error::other("planned reregistration rejection"))
    }

    fn deregister(&mut self, _registry: &Registry) -> io::Result<()> {
        Err(io::Error::other("planned deregistration rejection"))
    }
}

fn resource(generation: u64) -> ResourceToken {
    ResourceToken::new(
        ResourceOwnerId::new(1),
        ResourceSlotId::new(0),
        ResourceGeneration::new(generation),
    )
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}

fn unsupported_interest() -> Interest {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        Interest::AIO
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        Interest::PRIORITY
    }
}
