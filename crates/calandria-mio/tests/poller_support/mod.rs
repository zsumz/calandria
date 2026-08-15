//! Platform-specific supported Mio interest used by public adapter tests.

use calandria::Interest;

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
