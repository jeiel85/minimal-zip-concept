pub mod cli;
pub mod error;
pub mod format;
pub mod rle;
pub mod checksum;
pub mod inspect;
pub mod huffman;
pub mod gui;

use error::MzcError;
use format::{
    MzcHeader, HEADER_SIZE_MZC1, HEADER_SIZE_MZC2, ALGORITHM_RLE, ALGORITHM_DICT,
    ALGORITHM_HYBRID, ALGORITHM_LZ77, VERSION_MZC1, VERSION_MZC4, VERSION_MZC5, FILTER_DELTA,
    FILTER_BCJ, FILTER_DYNAMIC_HUFFMAN,
};
use rle::{
    Dictionary, build_dictionary, rle_compress_hybrid, rle_decompress_hybrid,
    rle_decompress_hybrid_mzc5,
};
use checksum::{calculate_sha256, bytes_to_hex};
use huffman::{huffman_compress, huffman_decompress, huffman_compress_dynamic, huffman_decompress_dynamic};
use cli::{CompressionMode, EntropyMode};
use rayon::prelude::*;

// 1MB (1,024,000 bytes) 청크 단위 크기 정의
pub const CHUNK_LIMIT: usize = 1_024_000;

/// MZC2의 병렬 청크 압축 단위를 모델링하는 이진 구조체 메타정보입니다.
/// 각 청크마다 헤더 [Original u32] + [Combined u32] + [Compressed u32] 뒤에 압축 데이터가 연이어 붙습니다.
pub const CHUNK_HEADER_SIZE: usize = 12;

/// 원본 바이트 데이터를 1MB 청크로 나누어 Rayon 스레드 풀에서 병렬 고속 압축하고,
/// 최종 56바이트 MZC2 고정 헤더와 병렬 청크 데이터가 결합된 압축 바이트 벡터를 반환합니다.
pub fn compress_bytes_v2(
    original: &[u8],
    mode: CompressionMode,
    entropy: EntropyMode,
    level: u8,
    delta: bool,
    bcj: bool,
) -> Vec<u8> {
    compress_bytes_v2_with_progress(original, mode, entropy, level, delta, bcj, |_, _, _, _| {})
}

/// GUI 또는 기타 통계를 위한 실시간 청크 압축 모니터링 기능이 보강된 압축 엔트리포인트입니다.
pub fn compress_bytes_v2_with_progress<F>(
    original: &[u8],
    mode: CompressionMode,
    entropy: EntropyMode,
    level: u8,
    delta: bool,
    bcj: bool,
    on_chunk_progress: F,
) -> Vec<u8>
where
    F: Fn(usize, usize, usize, f64) + Send + Sync + Clone,
{
    if original.is_empty() {
        // 빈 파일 처리: 56바이트의 빈 헤더만 방출
        let sha256 = calculate_sha256(original);
        let header = MzcHeader::new_v5(
            ALGORITHM_HYBRID | (if delta { FILTER_DELTA } else { 0 }) | (if bcj { FILTER_BCJ } else { 0 }),
            0,
            0,
            0,
            sha256
        );
        return header.to_bytes();
    }

    // 1. 원본 데이터를 1MB 청크 단위 슬라이스들로 분할하여 벡터에 참조 수집
    let mut chunks = Vec::new();
    let mut pos = 0;
    let n = original.len();
    while pos < n {
        let end = std::cmp::min(pos + CHUNK_LIMIT, n);
        chunks.push(&original[pos..end]);
        pos = end;
    }

    // 2. Rayon 멀티스레드 병렬 압축 맵 수행 (par_iter() 발동)
    let compressed_chunks: Vec<Vec<u8>> = chunks
        .par_iter()
        .enumerate()
        .map(|(chunk_idx, &chunk)| {
            let start_time = std::time::Instant::now();
            let chunk_orig_size = chunk.len() as u32;

            // 동작 모드 결정
            let alg_type = match mode {
                CompressionMode::Rle => ALGORITHM_RLE,
                CompressionMode::Dict => ALGORITHM_DICT,
                CompressionMode::Hybrid => ALGORITHM_HYBRID,
                CompressionMode::Lz77 => ALGORITHM_LZ77,
            };

            // A. 전처리 필터 적용 (BCJ 첫번째, Delta 두번째)
            let mut processed_chunk = chunk.to_vec();
            if bcj {
                rle::apply_bcj_filter(&mut processed_chunk);
            }
            if delta {
                rle::apply_delta_filter(&mut processed_chunk);
            }

            // B. 사전 추출 및 매칭 (Rle 단독 모드가 아니면 사전 생성)
            let dict = if alg_type != ALGORITHM_RLE {
                build_dictionary(&processed_chunk)
            } else {
                Dictionary::new()
            };
            let dict_bytes = dict.to_bytes();

            // C. RLE 하이브리드 압축 수행 (MZC5의 경우 bit-packed 구조 활용)
            let config = rle::CompressionConfig::from_level(level);
            let blocks = rle::compress_to_blocks(&processed_chunk, &dict, alg_type, &config);
            let rle_payload = rle::serialize_blocks_v5(&blocks);

            // D. 사전과 RLE 페이로드 결합
            let mut combined = dict_bytes;
            combined.extend_from_slice(&rle_payload);

            let chunk_comb_size = combined.len() as u32;

            // E. 엔트로피 코딩 추가 압축 여부 판정
            let final_payload = if entropy == EntropyMode::Huffman {
                huffman_compress(&combined)
            } else if entropy == EntropyMode::Dynamic {
                huffman_compress_dynamic(&combined)
            } else {
                combined
            };

            let chunk_comp_size = final_payload.len() as u32;

            // 실시간 콜백 송출
            let duration_secs = start_time.elapsed().as_secs_f64();
            on_chunk_progress(chunk_idx, chunk_orig_size as usize, chunk_comp_size as usize, duration_secs);

            // F. 청크 헤더 조립: [Original u32] [Combined u32] [Compressed u32]
            let mut chunk_bin = Vec::with_capacity(CHUNK_HEADER_SIZE + final_payload.len());
            chunk_bin.extend_from_slice(&chunk_orig_size.to_le_bytes());
            chunk_bin.extend_from_slice(&chunk_comb_size.to_le_bytes());
            chunk_bin.extend_from_slice(&chunk_comp_size.to_le_bytes());
            chunk_bin.extend_from_slice(&final_payload);

            chunk_bin
        })
        .collect();

    // 3. 병렬 처리된 청크 바이트들을 순서대로 하나의 페이로드 버퍼로 병합
    let mut payload_buffer = Vec::new();
    for chunk in compressed_chunks {
        payload_buffer.extend_from_slice(&chunk);
    }

    // 4. 원본 전체의 SHA-256 체크섬 구하기
    let total_sha256 = calculate_sha256(original);

    // 5. MZC5 헤더 생성 및 이진 조립
    let core_alg = match (mode, entropy) {
        (CompressionMode::Rle, EntropyMode::None) => ALGORITHM_RLE, // RLE 단독
        (CompressionMode::Lz77, _) => ALGORITHM_LZ77, // LZ77 하이브리드 모드
        _ => ALGORITHM_HYBRID, // 하이브리드 고도화 모드
    };

    let algorithm_type_flag = core_alg
        | (if delta { FILTER_DELTA } else { 0 })
        | (if bcj { FILTER_BCJ } else { 0 })
        | (if entropy == EntropyMode::Dynamic { FILTER_DYNAMIC_HUFFMAN } else { 0 });

    let header = MzcHeader::new_v5(
        algorithm_type_flag,
        original.len() as u64,
        payload_buffer.len() as u64,
        0,
        total_sha256,
    );

    let mut final_output = header.to_bytes();
    final_output.extend_from_slice(&payload_buffer);

    final_output
}

/// MZC 압축 바이너리 전체를 읽어와 MZC1~MZC5 포맷을 자동 감별하고,
/// Rayon 스레드 풀을 동원하여 멀티스레드로 각 청크를 병렬 해제하여 완벽 복원합니다.
pub fn decompress_bytes_v2(mzc_bytes: &[u8]) -> Result<Vec<u8>, MzcError> {
    if mzc_bytes.len() < 4 {
        return Err(MzcError::TruncatedHeader { read_bytes: mzc_bytes.len() });
    }

    // 1. 헤더 복구 및 이중 분기
    let header = MzcHeader::from_bytes(mzc_bytes)?;

    if header.version == VERSION_MZC1 {
        // ================== MZC1 하위 호환 Decompress ==================
        let payload = &mzc_bytes[HEADER_SIZE_MZC1..];
        if payload.len() != header.payload_size as usize {
            return Err(MzcError::TruncatedBlock {
                expected: header.payload_size as usize,
                found: payload.len(),
            });
        }
        // 옛날 RLE 복원 알고리즘 호출
        let decompressed = rle::rle_decompress(payload)?;
        
        // 크기 및 체크섬 매칭 검사
        if decompressed.len() as u64 != header.original_size {
            return Err(MzcError::OriginalSizeMismatch {
                expected: header.original_size,
                found: decompressed.len() as u64,
            });
        }
        let computed_sha256 = calculate_sha256(&decompressed);
        if computed_sha256 != header.original_sha256 {
            return Err(MzcError::ChecksumMismatch {
                expected: bytes_to_hex(&header.original_sha256),
                found: bytes_to_hex(&computed_sha256),
            });
        }
        return Ok(decompressed);
    }

    // ================== MZC2~MZC5 최신 병렬 Decompress ==================
    if header.original_size == 0 {
        return Ok(Vec::new());
    }

    let payload_area = &mzc_bytes[HEADER_SIZE_MZC2..];
    if payload_area.len() != header.payload_size as usize {
        return Err(MzcError::TruncatedBlock {
            expected: header.payload_size as usize,
            found: payload_area.len(),
        });
    }

    // A. 페이로드 영역을 쪼개어 각 청크 바이트 슬라이스 추출
    let mut chunk_slices = Vec::new();
    let mut pos = 0;
    let n = payload_area.len();

    while pos < n {
        if pos + CHUNK_HEADER_SIZE > n {
            return Err(MzcError::TruncatedBlock {
                expected: CHUNK_HEADER_SIZE,
                found: n - pos,
            });
        }

        // 청크 헤더 파싱
        let orig_size_bytes: [u8; 4] = payload_area[pos..pos + 4].try_into().unwrap();
        let comb_size_bytes: [u8; 4] = payload_area[pos + 4..pos + 8].try_into().unwrap();
        let comp_size_bytes: [u8; 4] = payload_area[pos + 8..pos + 12].try_into().unwrap();
        
        let chunk_orig_size = u32::from_le_bytes(orig_size_bytes) as usize;
        let chunk_comb_size = u32::from_le_bytes(comb_size_bytes) as usize;
        let chunk_comp_size = u32::from_le_bytes(comp_size_bytes) as usize;

        pos += CHUNK_HEADER_SIZE;

        if pos + chunk_comp_size > n {
            return Err(MzcError::TruncatedBlock {
                expected: chunk_comp_size,
                found: n - pos,
            });
        }

        let chunk_data = &payload_area[pos..pos + chunk_comp_size];
        chunk_slices.push((chunk_data, chunk_orig_size, chunk_comb_size));
        pos += chunk_comp_size;
    }

    // B. Rayon 멀티스레드로 각 청크를 동시 병렬 디코딩 (par_iter() 발동!)
    let decompressed_chunks: Result<Vec<Vec<u8>>, MzcError> = chunk_slices
        .par_iter()
        .map(|&(chunk_data, chunk_orig_size, chunk_comb_size)| {
            // 엔트로피 압축 해제 여부 자동 판독
            let is_dynamic = header.version == VERSION_MZC4
                || (header.version == VERSION_MZC5 && (header.algorithm_type & FILTER_DYNAMIC_HUFFMAN) != 0);

            let unhuffman = if is_dynamic {
                huffman_decompress_dynamic(chunk_data, chunk_comb_size)?
            } else if chunk_data.len() != chunk_comb_size {
                huffman_decompress(chunk_data, chunk_comb_size)?
            } else {
                chunk_data.to_vec()
            };

            // 만약 unhuffman 결과가 여전히 너무 짧다면 에러
            if unhuffman.len() < 2 {
                // RLE raw로 가정
                return Ok(unhuffman);
            }

            // 사전 복구 시도
            let dict = match Dictionary::from_bytes(&unhuffman) {
                Ok(d) => d,
                Err(_) => {
                    // MZC1 RLE direct 폴백 상황 처리
                    let rle_decomp = rle::rle_decompress(&unhuffman)?;
                    return Ok(rle_decomp);
                }
            };

            let dict_bytes_len = dict.to_bytes().len();
            if dict_bytes_len > unhuffman.len() {
                return Err(MzcError::CorruptDictionary);
            }

            let rle_payload = &unhuffman[dict_bytes_len..];

            // 디코딩 알고리즘 식별
            let core_alg = if header.version == VERSION_MZC5 {
                header.algorithm_type & 0x0F
            } else {
                header.algorithm_type
            };

            let alg_flag = if core_alg == ALGORITHM_LZ77 {
                ALGORITHM_LZ77
            } else if dict.entries.is_empty() {
                ALGORITHM_RLE
            } else {
                ALGORITHM_HYBRID
            };

            let mut decompressed_chunk = if header.version == VERSION_MZC5 {
                rle_decompress_hybrid_mzc5(rle_payload, &dict, alg_flag, chunk_orig_size)?
            } else {
                rle_decompress_hybrid(rle_payload, &dict, alg_flag, chunk_orig_size)?
            };

            // MZC5인 경우 역전처리 필터 적용 (Delta 역필터 첫번째, BCJ 역필터 두번째)
            if header.version == VERSION_MZC5 {
                let has_delta = (header.algorithm_type & FILTER_DELTA) != 0;
                let has_bcj = (header.algorithm_type & FILTER_BCJ) != 0;
                if has_delta {
                    rle::inverse_delta_filter(&mut decompressed_chunk);
                }
                if has_bcj {
                    rle::inverse_bcj_filter(&mut decompressed_chunk);
                }
            }

            if decompressed_chunk.len() != chunk_orig_size {
                return Err(MzcError::OriginalSizeMismatch {
                    expected: chunk_orig_size as u64,
                    found: decompressed_chunk.len() as u64,
                });
            }

            Ok(decompressed_chunk)
        })
        .collect();

    let decompressed_chunks = decompressed_chunks?;

    // C. 복원된 스레드별 청크들을 원래 순서로 순차 병합
    let mut restored_bytes = Vec::with_capacity(header.original_size as usize);
    for chunk in decompressed_chunks {
        restored_bytes.extend_from_slice(&chunk);
    }

    // D. 최종 크기 및 SHA-256 검사 수행
    if restored_bytes.len() as u64 != header.original_size {
        return Err(MzcError::OriginalSizeMismatch {
            expected: header.original_size,
            found: restored_bytes.len() as u64,
        });
    }

    let computed_sha256 = calculate_sha256(&restored_bytes);
    if computed_sha256 != header.original_sha256 {
        return Err(MzcError::ChecksumMismatch {
            expected: bytes_to_hex(&header.original_sha256),
            found: bytes_to_hex(&computed_sha256),
        });
    }

    Ok(restored_bytes)
}
