use mzc::checksum::calculate_sha256;
use mzc::cli::{CompressionMode, EntropyMode};

// 헬퍼 함수: 압축 후 복원하여 원본 데이터와 SHA-256 해시가 완벽히 같은지 테스트합니다.
fn assert_roundtrip(original: &[u8]) {
    let original_hash = calculate_sha256(original);

    // ================== 1. MZC2 Hybrid + Huffman (2차 엔트로피 포함) 검증 ==================
    let compressed_hybrid = mzc::compress_bytes_v2(
        original,
        CompressionMode::Hybrid,
        EntropyMode::Huffman,
        6,
        false,
        false,
        false,
        false,
    );
    let restored_hybrid = mzc::decompress_bytes_v2(&compressed_hybrid)
        .expect("MZC2 하이브리드+허프만 압축 해제 실패");

    assert_eq!(
        original.len(),
        restored_hybrid.len(),
        "하이브리드: 원본 크기와 복원 크기가 다릅니다."
    );
    assert_eq!(
        original,
        restored_hybrid.as_slice(),
        "하이브리드: 원본 바이트와 복원 바이트가 불일치합니다."
    );
    assert_eq!(
        original_hash,
        calculate_sha256(&restored_hybrid),
        "하이브리드: 체크섬 불일치."
    );

    // ================== 2. MZC2 Rle + None (MZC1 호환용 RLE 단독) 검증 ==================
    let compressed_rle = mzc::compress_bytes_v2(
        original,
        CompressionMode::Rle,
        EntropyMode::None,
        6,
        false,
        false,
        false,
        false,
    );
    let restored_rle =
        mzc::decompress_bytes_v2(&compressed_rle).expect("MZC2 RLE-only 압축 해제 실패");

    assert_eq!(
        original.len(),
        restored_rle.len(),
        "RLE: 원본 크기와 복원 크기가 다릅니다."
    );
    assert_eq!(
        original,
        restored_rle.as_slice(),
        "RLE: 원본 바이트와 복원 바이트가 불일치합니다."
    );
    assert_eq!(
        original_hash,
        calculate_sha256(&restored_rle),
        "RLE: 체크섬 불일치."
    );

    // ================== 3. MZC3 LZ77 + Huffman (백레퍼런스 + 허프만) 검증 ==================
    let compressed_lz77_huff = mzc::compress_bytes_v2(
        original,
        CompressionMode::Lz77,
        EntropyMode::Huffman,
        6,
        false,
        false,
        false,
        false,
    );
    let restored_lz77_huff =
        mzc::decompress_bytes_v2(&compressed_lz77_huff).expect("MZC3 LZ77+허프만 압축 해제 실패");

    assert_eq!(
        original.len(),
        restored_lz77_huff.len(),
        "LZ77+허프만: 원본 크기와 복원 크기가 다릅니다."
    );
    assert_eq!(
        original,
        restored_lz77_huff.as_slice(),
        "LZ77+허프만: 원본 바이트와 복원 바이트가 불일치합니다."
    );
    assert_eq!(
        original_hash,
        calculate_sha256(&restored_lz77_huff),
        "LZ77+허프만: 체크섬 불일치."
    );

    // ================== 4. MZC3 LZ77 + None (백레퍼런스 단독) 검증 ==================
    let compressed_lz77_none = mzc::compress_bytes_v2(
        original,
        CompressionMode::Lz77,
        EntropyMode::None,
        6,
        false,
        false,
        false,
        false,
    );
    let restored_lz77_none =
        mzc::decompress_bytes_v2(&compressed_lz77_none).expect("MZC3 LZ77 단독 압축 해제 실패");

    assert_eq!(
        original.len(),
        restored_lz77_none.len(),
        "LZ77 단독: 원본 크기와 복원 크기가 다릅니다."
    );
    assert_eq!(
        original,
        restored_lz77_none.as_slice(),
        "LZ77 단독: 원본 바이트와 복원 바이트가 불일치합니다."
    );
    assert_eq!(
        original_hash,
        calculate_sha256(&restored_lz77_none),
        "LZ77 단독: 체크섬 불일치."
    );

    // ================== 5. MZC4 LZ77 + Dynamic (백레퍼런스 + 동적 허프만) 검증 ==================
    let compressed_lz77_dyn = mzc::compress_bytes_v2(
        original,
        CompressionMode::Lz77,
        EntropyMode::Dynamic,
        6,
        false,
        false,
        false,
        false,
    );
    let restored_lz77_dyn = mzc::decompress_bytes_v2(&compressed_lz77_dyn)
        .expect("MZC4 LZ77+동적허프만 압축 해제 실패");

    assert_eq!(
        original.len(),
        restored_lz77_dyn.len(),
        "LZ77+동적허프만: 원본 크기와 복원 크기가 다릅니다."
    );
    assert_eq!(
        original,
        restored_lz77_dyn.as_slice(),
        "LZ77+동적허프만: 원본 바이트와 복원 바이트가 불일치합니다."
    );
    assert_eq!(
        original_hash,
        calculate_sha256(&restored_lz77_dyn),
        "LZ77+동적허프만: 체크섬 불일치."
    );

    // ================== 6. MZC4 Hybrid + Dynamic (RLE 하이브리드 + 동적 허프만) 검증 ==================
    let compressed_hybrid_dyn = mzc::compress_bytes_v2(
        original,
        CompressionMode::Hybrid,
        EntropyMode::Dynamic,
        6,
        false,
        false,
        false,
        false,
    );
    let restored_hybrid_dyn = mzc::decompress_bytes_v2(&compressed_hybrid_dyn)
        .expect("MZC4 하이브리드+동적허프만 압축 해제 실패");

    assert_eq!(
        original.len(),
        restored_hybrid_dyn.len(),
        "하이브리드+동적허프만: 원본 크기와 복원 크기가 다릅니다."
    );
    assert_eq!(
        original,
        restored_hybrid_dyn.as_slice(),
        "하이브리드+동적허프만: 원본 바이트와 복원 바이트가 불일치합니다."
    );
    assert_eq!(
        original_hash,
        calculate_sha256(&restored_hybrid_dyn),
        "하이브리드+동적허프만: 체크섬 불일치."
    );

    // ================== 7. MZC5 Advanced Features (Level 9 + Delta + BCJ) 검증 ==================
    let compressed_mzc5 = mzc::compress_bytes_v2(
        original,
        CompressionMode::Lz77,
        EntropyMode::Dynamic,
        9,
        true,
        true,
        false,
        false,
    );
    let restored_mzc5 =
        mzc::decompress_bytes_v2(&compressed_mzc5).expect("MZC5 고도화 압축 해제 실패");

    assert_eq!(
        original.len(),
        restored_mzc5.len(),
        "MZC5: 원본 크기와 복원 크기가 다릅니다."
    );
    assert_eq!(
        original,
        restored_mzc5.as_slice(),
        "MZC5: 원본 바이트와 복원 바이트가 불일치합니다."
    );
    assert_eq!(
        original_hash,
        calculate_sha256(&restored_mzc5),
        "MZC5: 체크섬 불일치."
    );

    // ================== 8. MZC7 Context Mixing (EntropyMode::Cm) 검증 ==================
    let compressed_cm = mzc::compress_bytes_v2(
        original,
        CompressionMode::Hybrid,
        EntropyMode::Cm,
        6,
        false,
        false,
        false,
        false,
    );
    let restored_cm =
        mzc::decompress_bytes_v2(&compressed_cm).expect("MZC7 Context Mixing 압축 해제 실패");

    assert_eq!(
        original.len(),
        restored_cm.len(),
        "MZC7 CM: 원본 크기와 복원 크기가 다릅니다."
    );
    assert_eq!(
        original,
        restored_cm.as_slice(),
        "MZC7 CM: 원본 바이트와 복원 바이트가 불일치합니다."
    );
    assert_eq!(
        original_hash,
        calculate_sha256(&restored_cm),
        "MZC7 CM: 체크섬 불일치."
    );
}

#[test]
fn test_empty_roundtrip() {
    assert_roundtrip(&[]);
}

#[test]
fn test_single_byte_roundtrip() {
    assert_roundtrip(&[b'A']);
}

#[test]
fn test_short_repeats() {
    assert_roundtrip(b"AAABBBBCCCCCD");
}

#[test]
fn test_complex_mixed_text() {
    let input =
        b"AAAAHello! This is a repeated text BBBB test. ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ!";
    assert_roundtrip(input);
}

#[test]
fn test_long_run_splitting() {
    let input = vec![b'A'; 70000];
    assert_roundtrip(&input);
}

#[test]
fn test_long_literal_splitting() {
    let mut input = Vec::with_capacity(70000);
    for idx in 0..70000 {
        input.push((idx % 3) as u8);
    }
    assert_roundtrip(&input);
}

#[test]
fn test_binary_roundtrip() {
    let mut binary_data = Vec::new();
    for i in 0..1000 {
        binary_data.push((i % 256) as u8);
    }
    assert_roundtrip(&binary_data);
}

#[test]
fn test_exact_block_limit_run() {
    let input = vec![b'B'; 65535];
    assert_roundtrip(&input);
}

#[test]
fn test_exact_block_limit_literal() {
    let mut input = Vec::with_capacity(65535);
    for i in 0..65535 {
        input.push((i % 3) as u8);
    }
    assert_roundtrip(&input);
}

#[test]
fn test_alternating_bytes() {
    let mut input = Vec::with_capacity(10000);
    for i in 0..10000 {
        input.push((i % 2) as u8);
    }
    assert_roundtrip(&input);
}

#[test]
fn test_large_mixed_stress() {
    // 1MB(1,048,576 bytes) 크기의 고성능 대형 혼합 스트레스 테스트.
    let mut input = Vec::with_capacity(1024 * 1024);

    // 1. 200,000바이트의 'X' 런
    input.resize(200000, b'X');

    // 2. 300,000바이트의 중복 없는 리터럴 영역
    for i in 0..300000 {
        input.push((i % 128) as u8);
    }

    // 3. 100,000바이트의 널 바이트(0x00) 런
    input.extend(std::iter::repeat(0x00).take(100000));

    // 4. 나머지 448,576바이트는 난수와 유사한 교차 패턴으로 구성
    for i in 0..448576 {
        input.push((i % 5) as u8);
    }

    assert_roundtrip(&input);
}

#[test]
fn test_dynamic_huffman_header_savings() {
    let input = b"Hello, this is a small text file designed to test header size savings!";
    let compressed_static = mzc::compress_bytes_v2(
        input,
        CompressionMode::Hybrid,
        EntropyMode::Huffman,
        6,
        false,
        false,
        false,
        false,
    );
    let compressed_dynamic = mzc::compress_bytes_v2(
        input,
        CompressionMode::Hybrid,
        EntropyMode::Dynamic,
        6,
        false,
        false,
        false,
        false,
    );

    // Static Huffman should be larger due to the 1024-byte frequency table header.
    // Dynamic Huffman header should be less than 60 bytes.
    assert!(
        compressed_static.len() > compressed_dynamic.len(),
        "Dynamic header should be smaller than 1024 bytes!"
    );
    println!(
        "Static size: {} bytes, Dynamic size: {} bytes",
        compressed_static.len(),
        compressed_dynamic.len()
    );
}

#[test]
fn test_huffman_dynamic_direct() {
    let input = b"Hello world! This is a test for dynamic huffman directly.";
    let compressed = mzc::huffman::huffman_compress_dynamic(input);
    let decompressed = mzc::huffman::huffman_decompress_dynamic(&compressed, input.len()).unwrap();
    assert_eq!(input, decompressed.as_slice());
}

#[test]
fn test_ans_direct() {
    let input =
        b"Hello world! This is a test for Table-based Asymmetric Numeral Systems (tANS) directly.";
    let compressed = mzc::ans::ans_compress(input).unwrap();
    let decompressed = mzc::ans::ans_decompress(&compressed, input.len()).unwrap();
    assert_eq!(input, decompressed.as_slice());
}

#[test]
fn test_ans_roundtrip() {
    let input = b"tANS compression roundtrip through the library pipeline test! Repeating repeating repeating...";
    let compressed = mzc::compress_bytes_v2(
        input,
        CompressionMode::Hybrid,
        EntropyMode::Ans,
        6,
        false,
        false,
        false,
        false,
    );
    let restored = mzc::decompress_bytes_v2(&compressed).expect("Ans decompress failed");
    assert_eq!(input, restored.as_slice());
}

#[test]
fn test_dictionary_training_roundtrip() {
    let sample1 = b"The quick brown fox jumps over the lazy dog.";
    let sample2 = b"A quick brown fox jumped over the lazy dogs and ran away.";
    let mut concat = Vec::new();
    concat.extend_from_slice(sample1);
    concat.extend_from_slice(sample2);

    let dict = mzc::rle::build_dictionary(&concat);
    let dict_bytes = dict.to_bytes();

    let input = b"The quick brown fox jumps over the lazy dog. A quick brown fox jumped!";
    let compressed = mzc::compress_bytes_v2_dict(
        input,
        CompressionMode::Hybrid,
        EntropyMode::Ans,
        6,
        false,
        false,
        false,
        false,
        Some(&dict_bytes),
    );

    let restored = mzc::decompress_bytes_v2_dict(&compressed, Some(&dict_bytes))
        .expect("Decompress with trained dict failed");
    assert_eq!(input, restored.as_slice());

    let restored_embedded =
        mzc::decompress_bytes_v2(&compressed).expect("Decompress with embedded dict failed");
    assert_eq!(input, restored_embedded.as_slice());
}

// ================== MZC7 신규 테스트 케이스 ==================

#[test]
fn test_png_filter_roundtrip() {
    // PNG Paeth 예측 필터링 검증용 그래디언트 형 가상 이미지 데이터 생성
    let mut input = vec![0u8; 4096];
    for i in 0..4096 {
        input[i] = (i % 256) as u8;
    }

    let compressed = mzc::compress_bytes_v2(
        &input,
        CompressionMode::Hybrid,
        EntropyMode::Huffman,
        6,
        false,
        false,
        true,
        false,
    );
    let restored = mzc::decompress_bytes_v2(&compressed).expect("PNG 필터 복원 실패");
    assert_eq!(input, restored);
}

#[test]
fn test_lpc_filter_roundtrip() {
    // 16비트 WAV PCM 오디오 LPC 예측 필터링 검증용 가상 신호 데이터 생성
    let mut input = vec![0u8; 2000];
    for i in 0..1000 {
        let val = (i * 15) as i16;
        let bytes = val.to_le_bytes();
        input[2 * i] = bytes[0];
        input[2 * i + 1] = bytes[1];
    }

    let compressed = mzc::compress_bytes_v2(
        &input,
        CompressionMode::Hybrid,
        EntropyMode::Huffman,
        6,
        false,
        false,
        false,
        true,
    );
    let restored = mzc::decompress_bytes_v2(&compressed).expect("LPC 필터 복원 실패");
    assert_eq!(input, restored);
}

#[test]
fn test_deflate_gzip_inflate_direct() {
    // RFC 1952 (GZIP) 헤더와 RFC 1951 (DEFLATE BTYPE=0 비압축 블록) 수동 조립
    let gzip_bytes = [
        0x1F, 0x8B, // Magic
        0x08, // CM (DEFLATE)
        0x00, // FLG
        0x00, 0x00, 0x00, 0x00, // MTIME
        0x00, // XFL
        0x03, // OS (Unix)
        // DEFLATE payload (BFINAL=1, BTYPE=00, LEN=5, NLEN=~5)
        0x01, 0x05, 0x00, 0xFA, 0xFF, 0x48, 0x65, 0x6C, 0x6C, 0x6F, // "Hello" 데이터
        // Footer (CRC32, ISIZE)
        0x82, 0x89, 0xD1, 0xF7, // "Hello"의 CRC32
        0x05, 0x00, 0x00, 0x00, // 크기 5
    ];
    let restored = mzc::deflate::gzip_decompress(&gzip_bytes).expect("GZIP 해독 실패");
    assert_eq!(restored, b"Hello");
}

#[test]
fn test_cm_stress() {
    // LCG pseudo-random number generator to avoid dependencies
    let mut seed = 12345u64;
    let mut next_random_byte = |seed_ref: &mut u64| -> u8 {
        *seed_ref = seed_ref.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*seed_ref >> 32) as u8
    };

    for test_idx in 0..100 {
        // Vary sizes from 1 to 2000 bytes
        let size = (test_idx * 17 + 13) % 2000 + 1;
        let mut input = vec![0u8; size];
        for j in 0..size {
            input[j] = next_random_byte(&mut seed);
        }

        let compressed = mzc::cm::cm_compress(&input).unwrap();
        let decompressed = mzc::cm::cm_decompress(&compressed, input.len()).unwrap();
        assert_eq!(
            input, decompressed,
            "CM stress failed at index {} with size {}",
            test_idx, size
        );
    }
}
