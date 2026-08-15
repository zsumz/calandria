//! Mio adapter failure identity, diagnostic, and source contracts.

use std::{error::Error, io, num::NonZeroUsize};

use calandria::{Interest, ResourceGeneration, ResourceOwnerId, ResourceSlotId, ResourceToken};
use calandria_mio::MioError;

#[test]
fn adapter_failures_preserve_tokens_interests_capacities_and_io_sources() {
    let token = ResourceToken::new(
        ResourceOwnerId::new(1),
        ResourceSlotId::new(2),
        ResourceGeneration::new(3),
    );
    let cases = [
        (
            MioError::RegistrationCapacity { limit: nonzero(4) },
            "Mio registration capacity of 4 was reached",
        ),
        (
            MioError::AlreadyRegistered { token },
            "resource token ResourceToken { owner: ResourceOwnerId(1), slot: ResourceSlotId(2), generation: ResourceGeneration(3) } is already registered",
        ),
        (
            MioError::NotRegistered { token },
            "resource token ResourceToken { owner: ResourceOwnerId(1), slot: ResourceSlotId(2), generation: ResourceGeneration(3) } is not registered",
        ),
        (
            MioError::UnsupportedInterest {
                interest: Interest::PRIORITY | Interest::AIO,
            },
            "Mio cannot express interest PRIORITY | AIO on this target",
        ),
        (
            MioError::TokenSpaceExhausted,
            "Mio backend token identities are exhausted",
        ),
        (
            MioError::DestinationTooSmall {
                required: nonzero(8),
                actual: nonzero(2),
            },
            "poll destination capacity 2 is smaller than required 8",
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
        assert!(Error::source(&error).is_none());
    }

    let error = MioError::from(io::Error::other("planned selector failure"));
    assert_eq!(
        error.to_string(),
        "Mio operation failed: planned selector failure"
    );
    assert_eq!(
        Error::source(&error).map(ToString::to_string),
        Some(String::from("planned selector failure"))
    );
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test capacity must be nonzero"))
}
