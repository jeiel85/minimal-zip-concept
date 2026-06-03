use mzc::error::MzcError;
use mzc::format::{MzcHeader, HEADER_SIZE_MZC1, HEADER_SIZE_MZC2};
use mzc::rle::{rle_decompress_hybrid, Dictionary};
use mzc::cli::{CompressionMode, EntropyMode};

#[test]
fn test_truncated_header() {
    let incomplete_data = vec![0u8; 30];
    let result = MzcHeader::from_bytes(&incomplete_data);
    assert!(result.is_err());
}

#[test]
fn test_invalid_magic_header() {
    let mut header_data = vec![0u8; HEADER_SIZE_MZC1];
    header_data[0..4].copy_from_slice(b"BAD1");
    header_data[4] = 0x01;
    header_data[5] = 0x01;

    let result = MzcHeader::from_bytes(&header_data);
    assert!(result.is_err());
}

#[test]
fn test_invalid_version() {
    let mut header_data = vec![0u8; HEADER_SIZE_MZC1];
    header_data[0..4].copy_from_slice(b"MZC1");
    header_data[4] = 0x02; // MZC1인데 버전 2 주입
    header_data[5] = 0x01;

    let result = MzcHeader::from_bytes(&header_data);
    assert!(result.is_err());
}

#[test]
fn test_invalid_algorithm_type() {
    let mut header_data = vec![0u8; HEADER_SIZE_MZC1];
    header_data[0..4].copy_from_slice(b"MZC1");
    header_data[4] = 0x01;
    header_data[5] = 0x09; // 존재하지 않는 알고리즘

    let result = MzcHeader::from_bytes(&header_data);
    assert!(result.is_err());
}

#[test]
fn test_truncated_payload_block() {
    // 10바이트 짜리 리터럴 블록을 예고했으나 3바이트만 남은 경우
    let mut payload = vec![0x00, 0x0A, 0x00];
    payload.extend_from_slice(b"ABC");

    let dict = Dictionary::new();
    let result = rle_decompress_hybrid(&payload, &dict, 0x03, 100);
    assert!(result.is_err());
    match result.unwrap_err() {
        MzcError::TruncatedBlock { expected, found } => {
            assert_eq!(expected, 10);
            assert_eq!(found, 3);
        }
        other => panic!("기대치 않은 에러 검출: {:?}", other),
    }
}

#[test]
fn test_corrupt_payload_checksum_mismatch() {
    // MZC2 고도화 하이브리드+허프만 압축 데이터에 대한 Bit-flip 오염 감지 테스트
    let original_data = b"AAAAHelloBBBBWorldCCCCRepeatedDataZZZZ! This is highly secret binary codes!";
    let compressed_data = mzc::compress_bytes_v2(original_data, CompressionMode::Hybrid, EntropyMode::Huffman, 6, false, false, false, false);
    
    // 정상은 당연히 디코딩 성공해야 함
    let restored = mzc::decompress_bytes_v2(&compressed_data).expect("정상 복원 실패");
    assert_eq!(restored, original_data);
    
    // 헤더(56바이트)를 비껴나간 페이로드 바이트 1개를 고의 오염시킵니다.
    let mut corrupt_data = compressed_data.clone();
    if corrupt_data.len() > HEADER_SIZE_MZC2 + 10 {
        corrupt_data[HEADER_SIZE_MZC2 + 10] ^= 0xFF; // 오염 주입
    }

    let result = mzc::decompress_bytes_v2(&corrupt_data);
    
    // 비트 오염 시 디코딩 실패 또는 체크섬 Mismatch가 완벽히 차단되어야 함
    assert!(result.is_err(), "오염된 페이로드는 무결성 검증을 절대 통과해선 안 됩니다.");
}

#[test]
fn test_mzc2_invalid_token_index() {
    // 유효 범위를 벗어난 사전 토큰 인덱스 공격 방어 검증
    // 사전 크기는 딱 2개 단어만 있는데, 페이로드에서 99번 인덱스 토큰 블록을 요청할 경우
    let mut dict = Dictionary::new();
    dict.entries.push(b"Hello".to_vec());
    dict.entries.push(b"World".to_vec());

    // Token Block [Type: 0x02] [Index: 99 (0x63 0x00)]
    let payload = vec![0x02, 0x63, 0x00];

    let result = rle_decompress_hybrid(&payload, &dict, 0x03, 100);
    assert!(result.is_err(), "이상 주소 토큰 참조는 디코더 선에서 즉시 차단되어야 합니다.");
    match result.unwrap_err() {
        MzcError::InvalidTokenIndex { index, max_valid } => {
            assert_eq!(index, 99);
            assert_eq!(max_valid, 2);
        }
        other => panic!("기대치 않은 에러: {:?}", other),
    }
}

#[test]
fn test_mzc2_corrupt_dictionary_layout() {
    // 사전 데이터가 파싱 도중 잘렸을 때의 안전 제어 확인
    // 사전 엔트리 개수는 u16(5개)라고 써놓고 본문은 3바이트만 제공하는 경우
    let corrupt_dict_bin = vec![0x05, 0x00, 0x04, 0x41]; // 5개 예고, 1개 단어만 부분 제공
    let result = Dictionary::from_bytes(&corrupt_dict_bin);
    assert!(result.is_err(), "손상된 사전 레이아웃은 파서가 즉시 차단해야 합니다.");
}
