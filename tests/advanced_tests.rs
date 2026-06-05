use mzc::cli::{CompressionMode, EntropyMode};
use std::io::Cursor;

#[test]
fn test_arm64_bcj_roundtrip() {
    // A mock ARM64 instruction block containing B and BL relative branches (0x14..=0x17, 0x94..=0x97 in the MSB)
    let mut data = vec![
        0x01, 0x00, 0x00, 0x14, // B label
        0x20, 0x03, 0x00, 0x94, // BL label
        0x00, 0x00, 0x00, 0x00, // standard data
        0xFF, 0xFF, 0xFF, 0x97, // BL negative offset
    ];
    let orig = data.clone();

    // Apply filters
    mzc::rle::apply_bcj_filter(&mut data);
    // Absolute addresses should be set, so the instructions should have changed
    assert_ne!(orig, data);

    // Inverse filters
    mzc::rle::inverse_bcj_filter(&mut data);
    // Should be identical to original
    assert_eq!(orig, data);
}

#[test]
fn test_riscv_bcj_roundtrip() {
    // A mock RISC-V JAL instruction (ends with 0x6F)
    let mut data = vec![
        0xef, 0xf0, 0x1f, 0xff, // jal x1, -2
        0x6f, 0x00, 0x00, 0x00, // jal x0, 0
        0x6f, 0xf0, 0xdf, 0xff, // jal x0, -32
        0xef, 0x00, 0x20, 0x00, // jal x1, 4
    ];
    let orig = data.clone();

    mzc::rle::apply_bcj_filter(&mut data);
    assert_ne!(orig, data);

    mzc::rle::inverse_bcj_filter(&mut data);
    assert_eq!(orig, data);
}

#[test]
fn test_delta_simd_correctness() {
    let mut data = vec![
        10, 12, 15, 19, 24, 30, 37, 45, 54, 64, 75, 87, 100, 114, 129, 145, 162, 180, 200,
    ];
    let orig = data.clone();

    // Run SIMD delta filter
    mzc::rle::apply_delta_filter(&mut data);

    // Check values manually
    let mut expected = orig.clone();
    for i in (1..expected.len()).rev() {
        expected[i] = expected[i].wrapping_sub(expected[i - 1]);
    }
    assert_eq!(data, expected);

    // Inverse
    mzc::rle::inverse_delta_filter(&mut data);
    assert_eq!(data, orig);
}

#[test]
fn test_lpc_simd_correctness() {
    // 32 bytes = 16 i16 audio samples
    let samples = vec![
        100i16, 120, 140, 180, 230, 300, 400, 520, 660, 820, 990, 376, 506, 656, 826, 996,
    ];
    let mut data = Vec::new();
    for s in samples {
        data.extend_from_slice(&s.to_le_bytes());
    }
    let orig = data.clone();

    // Run SIMD LPC filter
    mzc::filters::apply_lpc_filter(&mut data);

    // Check values against standard inverse
    mzc::filters::inverse_lpc_filter(&mut data);
    assert_eq!(data, orig);
}

#[test]
fn test_png_simd_correctness() {
    // Generate a 4096-byte array (2 rows of 2048 bytes) with gradient values
    let mut data = vec![0u8; 4096];
    for i in 0..4096 {
        data[i] = (i % 251) as u8;
    }
    let orig = data.clone();

    // 1. Run with SIMD enabled
    mzc::ENABLE_SIMD.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut simd_data = data.clone();
    mzc::filters::apply_png_filter(&mut simd_data);

    // 2. Run with SIMD disabled
    mzc::ENABLE_SIMD.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut scalar_data = data.clone();
    mzc::filters::apply_png_filter(&mut scalar_data);

    // Both should produce identical filtered output
    assert_eq!(simd_data, scalar_data);

    // 3. Verify undo works
    mzc::filters::inverse_png_filter(&mut simd_data);
    assert_eq!(simd_data, orig);

    // Restore SIMD setting
    mzc::ENABLE_SIMD.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[test]
fn test_streaming_roundtrip() {
    // Let's test a larger text block (approx 50KB) to verify block-based streaming
    let mut text = String::new();
    for i in 0..1000 {
        text.push_str(&format!(
            "{}: Hello Context Mixing and Zero-Copy streaming! ",
            i
        ));
    }
    let original_bytes = text.into_bytes();

    let mut input_cursor = Cursor::new(original_bytes.clone());
    let mut compressed_cursor = Cursor::new(Vec::new());

    // Run compress stream
    mzc::compress_stream(
        &mut input_cursor,
        &mut compressed_cursor,
        CompressionMode::Hybrid,
        EntropyMode::Cm,
        6,
        true,
        true,
        false,
        false,
        false, // bwt
        None,
    )
    .expect("Streaming compression failed");

    // Run decompress stream
    let compressed_bytes = compressed_cursor.into_inner();
    let mut compressed_reader = Cursor::new(compressed_bytes);
    let mut decompressed_writer = Cursor::new(Vec::new());

    mzc::decompress_stream(&mut compressed_reader, &mut decompressed_writer, None)
        .expect("Streaming decompression failed");

    let restored_bytes = decompressed_writer.into_inner();
    assert_eq!(original_bytes.len(), restored_bytes.len());
    assert_eq!(original_bytes, restored_bytes);
}

#[test]
fn test_bwt_mtf_roundtrip() {
    let mut data = b"banana-split-with-extra-banana-and-split".to_vec();
    let orig = data.clone();

    // Test BWT + MTF filter roundtrip
    mzc::filters::apply_bwt_filter(&mut data);
    assert_ne!(orig, data); // Should be transformed

    mzc::filters::inverse_bwt_filter(&mut data);
    assert_eq!(orig, data); // Should be restored exactly
}

#[test]
fn test_bwt_compress_roundtrip() {
    let original_bytes = b"Burrows-Wheeler Transform (BWT) rearranges a character string into runs of similar characters. This is extremely useful for compression.".repeat(10);
    
    // Compress with BWT enabled
    let compressed = mzc::compress_bytes_v2_with_progress_dict(
        &original_bytes,
        CompressionMode::Lz77,
        EntropyMode::Cm,
        6,
        false,
        false,
        false,
        false,
        true, // bwt enabled
        None,
        |_, _, _, _| {},
    );
    assert!(!compressed.is_empty());

    // Decompress and verify
    let decompressed = mzc::decompress_bytes_v2(&compressed).expect("BWT decompression failed");
    assert_eq!(original_bytes.len(), decompressed.len());
    assert_eq!(original_bytes, decompressed);
}

