//! Portable entropy algorithm vectors and independent-stream tests.

use calandria_sim::{EntropySeed, EntropyStreamId, SplitMix64};

#[test]
fn splitmix64_version_one_matches_golden_vectors() {
    let mut entropy = SplitMix64::new(EntropySeed::new(0), EntropyStreamId::new(0));
    let actual = core::array::from_fn::<_, 5, _>(|_| entropy.next_u64());

    assert_eq!(SplitMix64::ALGORITHM_VERSION, 1);
    assert_eq!(
        actual,
        [
            0x1df5_df97_578d_90c0,
            0xbb0e_8eb9_91d7_d0f7,
            0x274e_2155_3f69_0adc,
            0x0f6f_b523_f192_5196,
            0x10ca_8539_0bfc_4e35,
        ]
    );
}

#[test]
fn stream_draws_do_not_perturb_another_namespace() {
    let seed = EntropySeed::new(0x0123_4567_89ab_cdef);
    let mut first = SplitMix64::new(seed, EntropyStreamId::new(7));
    let mut replay = SplitMix64::new(seed, EntropyStreamId::new(7));
    let mut other = SplitMix64::new(seed, EntropyStreamId::new(8));

    let expected = core::array::from_fn::<_, 5, _>(|_| replay.next_u64());
    let _unrelated = core::array::from_fn::<_, 11, _>(|_| other.next_u64());
    let actual = core::array::from_fn::<_, 5, _>(|_| first.next_u64());

    assert_eq!(actual, expected);
    assert_ne!(actual[0], other.next_u64());
}
