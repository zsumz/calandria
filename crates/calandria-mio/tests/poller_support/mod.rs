//! Platform-specific interests, identities, and rejection sources for adapter tests.

use std::{io, num::NonZeroUsize};

use calandria::{Interest, ResourceGeneration, ResourceOwnerId, ResourceSlotId, ResourceToken};
use mio::{Interest as MioInterest, Registry, Token, event::Source};

#[derive(Debug)]
pub(crate) struct RejectLaterSource;

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
pub(crate) struct RejectingSource;

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

pub(crate) fn resource(generation: u64) -> ResourceToken {
    ResourceToken::new(
        ResourceOwnerId::new(1),
        ResourceSlotId::new(0),
        ResourceGeneration::new(generation),
    )
}

pub(crate) fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}

pub(crate) fn unsupported_interest() -> Interest {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        Interest::AIO
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        Interest::PRIORITY
    }
}

pub(crate) fn supported_compound_interest() -> Interest {
    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
    ))]
    {
        Interest::READABLE | Interest::WRITABLE | Interest::AIO
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        Interest::READABLE | Interest::WRITABLE | Interest::PRIORITY
    }
    #[cfg(not(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
    )))]
    {
        Interest::READABLE | Interest::WRITABLE
    }
}
