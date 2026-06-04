use mzc::cli::{CompressionMode, EntropyMode};

// LCG pseudo-random generator for reproducible fuzz inputs
fn get_pseudo_random_bytes(seed: &mut u64, len: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(len);
    for _ in 0..len {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        bytes.push((*seed >> 32) as u8);
    }
    bytes
}

#[test]
fn test_decompress_bytes_v2_robustness_on_truncation() {
    let original = b"Hello, this is a highly repetitive test input to produce a valid compressed MZC7 binary. Hello, this is a highly repetitive test input to produce a valid compressed MZC7 binary.";
    
    // Generate valid compressed buffers for different configurations
    let configs = [
        (CompressionMode::Hybrid, EntropyMode::Huffman),
        (CompressionMode::Hybrid, EntropyMode::Dynamic),
        (CompressionMode::Hybrid, EntropyMode::Ans),
        (CompressionMode::Hybrid, EntropyMode::Cm),
        (CompressionMode::Lz77, EntropyMode::Cm),
    ];

    for &(mode, entropy) in &configs {
        let compressed = mzc::compress_bytes_v2(original, mode, entropy, 6, true, true, false, false);
        assert!(!compressed.is_empty());

        // Test every truncation level from 0 to compressed.len()
        for limit in 0..compressed.len() {
            let truncated = &compressed[..limit];
            let result = std::panic::catch_unwind(|| {
                let _ = mzc::decompress_bytes_v2(truncated);
            });
            assert!(result.is_ok(), "Panic detected on truncated MZC input with mode={:?}, entropy={:?} at limit {}", mode, entropy, limit);
        }
    }
}

#[test]
fn test_decompress_bytes_v2_robustness_on_mutations() {
    let original = b"Stress testing mutations. ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ!";
    let compressed = mzc::compress_bytes_v2(original, CompressionMode::Hybrid, EntropyMode::Cm, 6, true, true, false, false);
    
    let mut seed = 54321u64;

    // Mutate individual bytes in the valid compressed stream
    for idx in 0..compressed.len() {
        let mut mutated = compressed.clone();
        
        // Try multiple mutated values for this byte position
        for _ in 0..3 {
            let rand_val = get_pseudo_random_bytes(&mut seed, 1)[0];
            mutated[idx] = rand_val;

            let result = std::panic::catch_unwind(|| {
                let _ = mzc::decompress_bytes_v2(&mutated);
            });
            assert!(result.is_ok(), "Panic detected on mutated byte at index {} in CM compressed binary", idx);
        }
    }
}

#[test]
fn test_decompress_bytes_v2_robustness_on_completely_random_data() {
    let mut seed = 98765u64;

    for test_len in [0, 1, 5, 50, 56, 100, 1000, 10000] {
        for _ in 0..20 {
            let random_data = get_pseudo_random_bytes(&mut seed, test_len);
            let result = std::panic::catch_unwind(|| {
                let _ = mzc::decompress_bytes_v2(&random_data);
            });
            assert!(result.is_ok(), "Panic detected on completely random MZC data of size {}", test_len);
        }
    }
}

#[test]
fn test_deflate_decompress_robustness() {
    let mut seed = 11111u64;

    // Test raw inflate and gzip_decompress on random byte inputs
    for test_len in [0, 1, 10, 18, 50, 500] {
        for _ in 0..50 {
            let random_data = get_pseudo_random_bytes(&mut seed, test_len);
            
            // raw inflate
            let result_raw = std::panic::catch_unwind(|| {
                let _ = mzc::deflate::inflate(&random_data);
            });
            assert!(result_raw.is_ok(), "Panic detected on raw inflate with random size {}", test_len);

            // gzip_decompress
            let result_gzip = std::panic::catch_unwind(|| {
                let _ = mzc::deflate::gzip_decompress(&random_data);
            });
            assert!(result_gzip.is_ok(), "Panic detected on gzip_decompress with random size {}", test_len);
        }
    }
}

#[test]
fn test_tans_decompress_robustness() {
    let mut seed = 22222u64;

    for test_len in [0, 1, 5, 20, 100, 1000] {
        for _ in 0..50 {
            let random_data = get_pseudo_random_bytes(&mut seed, test_len);
            let result = std::panic::catch_unwind(|| {
                let _ = mzc::ans::ans_decompress(&random_data, 100);
            });
            assert!(result.is_ok(), "Panic detected on ans_decompress with random size {}", test_len);
        }
    }
}

#[test]
fn test_cm_decompress_robustness() {
    let mut seed = 33333u64;

    for test_len in [0, 1, 5, 20, 100, 500] {
        for _ in 0..30 {
            let random_data = get_pseudo_random_bytes(&mut seed, test_len);
            let result = std::panic::catch_unwind(|| {
                // Ensure no panic even with different expected original size values
                let _ = mzc::cm::cm_decompress(&random_data, 100);
            });
            assert!(result.is_ok(), "Panic detected on cm_decompress with random size {}", test_len);
        }
    }
}
