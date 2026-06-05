use mzc::cli::{CompressionMode, EntropyMode};
use proptest::prelude::*;

// Define strategy to generate a random CompressionMode
fn compression_mode_strategy() -> impl Strategy<Value = CompressionMode> {
    prop_oneof![
        Just(CompressionMode::Rle),
        Just(CompressionMode::Dict),
        Just(CompressionMode::Hybrid),
        Just(CompressionMode::Lz77),
    ]
}

// Define strategy to generate a random EntropyMode
fn entropy_mode_strategy() -> impl Strategy<Value = EntropyMode> {
    prop_oneof![
        Just(EntropyMode::None),
        Just(EntropyMode::Huffman),
        Just(EntropyMode::Dynamic),
        Just(EntropyMode::Ans),
        Just(EntropyMode::Cm),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn test_random_roundtrip_without_password(
        ref data in prop::collection::vec(any::<u8>(), 0..2000),
        mode in compression_mode_strategy(),
        entropy in entropy_mode_strategy(),
        level in 1..=9u8,
        delta in any::<bool>(),
        bcj in any::<bool>(),
        png in any::<bool>(),
        lpc in any::<bool>(),
    ) {
        // Cm is only fully supported or robust with Hybrid/Lz77, so we adjust modes if needed
        // to avoid invalid combinations or keep tests realistic.
        let (mode, entropy) = match (mode, entropy) {
            (CompressionMode::Rle, EntropyMode::Cm) => (CompressionMode::Hybrid, EntropyMode::Cm),
            (CompressionMode::Dict, EntropyMode::Cm) => (CompressionMode::Hybrid, EntropyMode::Cm),
            (m, e) => (m, e),
        };

        // Dict compression needs a dictionary, so if Dict mode is chosen without dict,
        // it falls back to Hybrid internally.

        let compressed = mzc::compress_bytes_v2_dict_password(
            data,
            mode,
            entropy,
            level,
            delta,
            bcj,
            png,
            lpc,
            None,
            None,
        );

        let decompressed = mzc::decompress_bytes_v2_dict_password(&compressed, None, None)
            .expect("Decompression failed");

        assert_eq!(data, &decompressed);
    }

    #[test]
    fn test_random_roundtrip_with_password(
        ref data in prop::collection::vec(any::<u8>(), 0..1000),
        ref password in "[a-zA-Z0-9]{4,16}",
        mode in compression_mode_strategy(),
        entropy in entropy_mode_strategy(),
        delta in any::<bool>(),
        bcj in any::<bool>(),
    ) {
        let (mode, entropy) = match (mode, entropy) {
            (CompressionMode::Rle, EntropyMode::Cm) => (CompressionMode::Hybrid, EntropyMode::Cm),
            (CompressionMode::Dict, EntropyMode::Cm) => (CompressionMode::Hybrid, EntropyMode::Cm),
            (m, e) => (m, e),
        };

        let compressed = mzc::compress_bytes_v2_dict_password(
            data,
            mode,
            entropy,
            5,
            delta,
            bcj,
            false,
            false,
            None,
            Some(password),
        );

        // Decompress with correct password
        let decompressed = mzc::decompress_bytes_v2_dict_password(&compressed, None, Some(password))
            .expect("Decompression with correct password failed");
        assert_eq!(data, &decompressed);

        // Decompress with wrong password should fail
        let wrong_password = format!("{}x", password);
        let decrypt_result = mzc::decompress_bytes_v2_dict_password(&compressed, None, Some(&wrong_password));
        assert!(decrypt_result.is_err());
    }
}
