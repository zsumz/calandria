//! Explicit portable entropy streams for deterministic scheduling and scenarios.

/// Fixed-width root entropy selected by a deterministic scenario.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntropySeed(u64);

impl EntropySeed {
    /// Creates an explicit root seed.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the fixed-width seed value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Caller-assigned namespace for one independently advanced entropy stream.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntropyStreamId(u64);

impl EntropyStreamId {
    /// Creates a stream namespace.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the fixed-width namespace value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// `SplitMix64` version 1 with explicit seed and stream derivation.
///
/// This generator is portable and reproducible, not cryptographically secure.
/// Its wrapping arithmetic and constants are part of the versioned algorithm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitMix64 {
    seed: EntropySeed,
    stream: EntropyStreamId,
    state: u64,
}

impl SplitMix64 {
    /// Public version of the generator and stream-derivation algorithm.
    pub const ALGORITHM_VERSION: u16 = 1;

    /// Derives one independent stream without consuming another stream.
    pub const fn new(seed: EntropySeed, stream: EntropyStreamId) -> Self {
        let namespace = mix(stream.0.wrapping_add(STREAM_DOMAIN));
        Self {
            seed,
            stream,
            state: seed.0 ^ namespace,
        }
    }

    /// Returns the root seed.
    pub const fn seed(self) -> EntropySeed {
        self.seed
    }

    /// Returns the stream namespace.
    pub const fn stream(self) -> EntropyStreamId {
        self.stream
    }

    /// Advances this stream exactly once and returns the next portable value.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GAMMA);
        mix(self.state)
    }
}

const GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;
const STREAM_DOMAIN: u64 = 0xd1b5_4a32_d192_ed03;

const fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
