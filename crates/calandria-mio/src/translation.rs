//! Platform-aware readiness translation without resource policy.

use calandria::{Interest, Readiness};

use crate::MioError;

pub(super) fn into_mio_interest(interest: Interest) -> Result<mio::Interest, MioError> {
    let mut mapped = None;
    let mut accepted = None;
    if interest.is_readable() {
        include_interest(
            &mut mapped,
            &mut accepted,
            Interest::READABLE,
            mio::Interest::READABLE,
        );
    }
    if interest.is_writable() {
        include_interest(
            &mut mapped,
            &mut accepted,
            Interest::WRITABLE,
            mio::Interest::WRITABLE,
        );
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if interest.is_priority() {
        include_interest(
            &mut mapped,
            &mut accepted,
            Interest::PRIORITY,
            mio::Interest::PRIORITY,
        );
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    if interest.is_priority() {
        return Err(MioError::UnsupportedInterest { interest });
    }

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
    ))]
    if interest.is_aio() {
        include_interest(
            &mut mapped,
            &mut accepted,
            Interest::AIO,
            mio::Interest::AIO,
        );
    }
    #[cfg(not(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
    )))]
    if interest.is_aio() {
        return Err(MioError::UnsupportedInterest { interest });
    }

    #[cfg(target_os = "freebsd")]
    if interest.is_lio() {
        include_interest(
            &mut mapped,
            &mut accepted,
            Interest::LIO,
            mio::Interest::LIO,
        );
    }
    #[cfg(not(target_os = "freebsd"))]
    if interest.is_lio() {
        return Err(MioError::UnsupportedInterest { interest });
    }

    let Some(mapped) = mapped else {
        return Err(MioError::UnsupportedInterest { interest });
    };
    let Some(accepted) = accepted else {
        return Err(MioError::UnsupportedInterest { interest });
    };
    if interest.remove(accepted).is_some() {
        return Err(MioError::UnsupportedInterest { interest });
    }
    Ok(mapped)
}

fn include_interest(
    target: &mut Option<mio::Interest>,
    accepted: &mut Option<Interest>,
    core: Interest,
    backend: mio::Interest,
) {
    *target = Some(match *target {
        Some(current) => current.add(backend),
        None => backend,
    });
    *accepted = Some(match *accepted {
        Some(current) => current.union(core),
        None => core,
    });
}

pub(super) fn readiness(event: &mio::event::Event) -> Readiness {
    let mut readiness = Readiness::EMPTY;
    if event.is_readable() {
        readiness = readiness.union(Readiness::READABLE);
    }
    if event.is_writable() {
        readiness = readiness.union(Readiness::WRITABLE);
    }
    if event.is_read_closed() {
        readiness = readiness.union(Readiness::READ_CLOSED);
    }
    if event.is_write_closed() {
        readiness = readiness.union(Readiness::WRITE_CLOSED);
    }
    if event.is_error() {
        readiness = readiness.union(Readiness::ERROR);
    }
    if event.is_priority() {
        readiness = readiness.union(Readiness::PRIORITY);
    }
    if event.is_aio() {
        readiness = readiness.union(Readiness::AIO);
    }
    if event.is_lio() {
        readiness = readiness.union(Readiness::LIO);
    }
    readiness
}
