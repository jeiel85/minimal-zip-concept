// [Rust 모듈 시스템 설명]
// - pub mod: 외부 파일(예: cli.rs, error.rs 등)을 현재 라이브러리의 하위 모듈로 등록하고 공개(pub)합니다.
// - 이를 통해 컴파일러는 이 하위 모듈 파일들을 함께 묶어서 컴파일합니다.
pub mod cli;
pub mod error;
pub mod format;
pub mod rle;
pub mod checksum;
pub mod inspect;
pub mod huffman;
pub mod ans;
pub mod cm;
pub mod filters;
pub mod deflate;
pub mod gui;

// [Rust 경로 수입 설명]
// - use: 다른 모듈에 선언되어 있는 구조체, 에러, 함수 등을 현재 파일의 범위(Scope) 안으로 가져와 축약어로 쓸 수 있게 만듭니다.
use error::MzcError;
use format::{
    MzcHeader, HEADER_SIZE_MZC1, HEADER_SIZE_MZC2, ALGORITHM_RLE, ALGORITHM_DICT,
    ALGORITHM_HYBRID, ALGORITHM_LZ77, VERSION_MZC1, VERSION_MZC4, VERSION_MZC5, VERSION_MZC6,
    VERSION_MZC7, FILTER_DELTA, FILTER_BCJ, FILTER_DYNAMIC_HUFFMAN, FILTER_ANS,
};
use rle::{
    Dictionary, build_dictionary, rle_decompress_hybrid,
    rle_decompress_hybrid_mzc5,
};
use checksum::{calculate_sha256, bytes_to_hex};
use huffman::{huffman_compress, huffman_decompress, huffman_compress_dynamic, huffman_decompress_dynamic};
use cli::{CompressionMode, EntropyMode};

// [Rust 병렬성 확장 설명]
// - rayon::prelude::*: Rayon은 Rust에서 CPU 멀티코어 병렬 처리를 아주 쉽게 할 수 있도록 돕는 대표적인 라이브러리입니다.
// - `*` 기호를 써서 병렬 반복자(Parallel Iterator) 기능들을 일괄 로드합니다.
use rayon::prelude::*;

// [Rust 기초 설명]
// - 숫자 표기 방식: `1_024_000`처럼 숫자 사이에 밑줄(_)을 넣으면 가독성이 향상됩니다. 컴파일러는 이를 일반 숫자 1024000과 동일하게 취급합니다.
// - MZC는 대용량 파일을 효율적으로 압축하기 위해 파일을 1MB 단위의 '청크(Chunk)'로 나누어 병렬 처리합니다.
pub const CHUNK_LIMIT: usize = 1_024_000;

/// MZC2 이후 병렬 청크 압축 단위를 모델링하는 이진 구조체 메타정보 크기입니다.
/// 각 청크마다 헤더 [Original u32] + [Combined u32] + [Compressed u32] (총 12바이트) 뒤에 압축 데이터가 연이어 붙습니다.
pub const CHUNK_HEADER_SIZE: usize = 12;

/// **원본 바이트 데이터를 1MB 청크로 나누어 Rayon 스레드 풀에서 병렬 고속 압축합니다.**
///
/// # Rust 개념 설명:
/// - `&[u8]`: 바이트 데이터를 가리키는 읽기 전용 슬라이스 참조자입니다. 복사 오버헤드 없이 원본 데이터를 효율적으로 다룹니다.
/// - `Vec<u8>`: 데이터를 동적으로 추가하거나 제거할 수 있는 힙(Heap) 메모리 할당 바이트 배열입니다.
pub fn compress_bytes_v2(
    original: &[u8],
    mode: CompressionMode,
    entropy: EntropyMode,
    level: u8,
    delta: bool,
    bcj: bool,
    png: bool,
    lpc: bool,
) -> Vec<u8> {
    compress_bytes_v2_dict(original, mode, entropy, level, delta, bcj, png, lpc, None)
}

/// **전역 공유 사전 데이터(dict_data)를 지원하는 버전 6 및 7 압축 엔트리포인트입니다.**
pub fn compress_bytes_v2_dict(
    original: &[u8],
    mode: CompressionMode,
    entropy: EntropyMode,
    level: u8,
    delta: bool,
    bcj: bool,
    png: bool,
    lpc: bool,
    dict_data: Option<&[u8]>,
) -> Vec<u8> {
    compress_bytes_v2_with_progress_dict(original, mode, entropy, level, delta, bcj, png, lpc, dict_data, |_, _, _, _| {})
}

/// **GUI 또는 기타 통계를 위한 실시간 청크 압축 모니터링 기능이 보강된 압축 엔트리포인트입니다.**
pub fn compress_bytes_v2_with_progress<F>(
    original: &[u8],
    mode: CompressionMode,
    entropy: EntropyMode,
    level: u8,
    delta: bool,
    bcj: bool,
    png: bool,
    lpc: bool,
    on_chunk_progress: F,
) -> Vec<u8>
where
    F: Fn(usize, usize, usize, f64) + Send + Sync + Clone,
{
    compress_bytes_v2_with_progress_dict(original, mode, entropy, level, delta, bcj, png, lpc, None, on_chunk_progress)
}

/// **GUI/통계 모니터링 및 전역 사전을 동시 지원하는 코어 압축 파이프라인 (MZC7 대응)**
///
/// # Rust 개념 설명:
/// - `where F: Fn(...) + Send + Sync + Clone`: 클로저(콜백 함수) `F`에 대한 제약 조건입니다.
///   * `Fn(...)`: 특정 형태의 인자를 받아 작동하는 함수/클로저 타입임을 명시합니다.
///   * `Send`: 이 클로저가 다른 스레드로 안전하게 복사/이동(Send)될 수 있음을 증명합니다.
///   * `Sync`: 여러 스레드에서 이 클로저를 동시 참조(`&F`)하여 실행해도 데이터 레이스(경합)가 없음(Sync)을 보장합니다.
///   * `Clone`: Rayon 병렬 스레드가 작업을 나누어 가질 때 클로저를 안전하게 복제할 수 있음을 나타냅니다.
pub fn compress_bytes_v2_with_progress_dict<F>(
    original: &[u8],
    mode: CompressionMode,
    entropy: EntropyMode,
    level: u8,
    delta: bool,
    bcj: bool,
    png: bool,
    lpc: bool,
    dict_data: Option<&[u8]>,
    on_chunk_progress: F,
) -> Vec<u8>
where
    F: Fn(usize, usize, usize, f64) + Send + Sync + Clone,
{
    // 빈 파일 예외 처리
    if original.is_empty() {
        let sha256 = calculate_sha256(original);
        // 빈 헤더를 생성하여 즉시 반환합니다.
        let header = MzcHeader::new_v6(
            ALGORITHM_HYBRID | (if delta { FILTER_DELTA } else { 0 }) | (if bcj { FILTER_BCJ } else { 0 }),
            0,
            0,
            0,
            sha256
        );
        return header.to_bytes();
    }

    // 전역 공유 사전 파싱 (Option 매칭 활용)
    let global_dict = if let Some(bytes) = dict_data {
        match Dictionary::from_bytes(bytes) {
            Ok(d) => Some(d),
            Err(_) => None,
        }
    } else {
        None
    };

    let global_dict_bytes = global_dict.as_ref().map(|d| d.to_bytes()).unwrap_or_default();
    let dictionary_size = global_dict_bytes.len() as u16;

    // 1. 원본 데이터를 1MB 청크 단위 슬라이스들로 분할
    let mut chunks = Vec::new();
    let mut pos = 0;
    let n = original.len();
    while pos < n {
        let end = std::cmp::min(pos + CHUNK_LIMIT, n);
        // [Rust 배열 슬라이싱]
        // - `&original[pos..end]`: 메모리 복사 없이, 원본 배열의 특정 구간만을 가리키는 포인터를 따서 벡터에 넣습니다.
        chunks.push(&original[pos..end]);
        pos = end;
    }

    // MZC7 조건 판단: Context Mixing 엔트로피나 미디어 전용 필터(PNG, LPC)가 켜진 경우 MZC7 규격 적용
    let is_v7 = entropy == EntropyMode::Cm || png || lpc;

    // 2. Rayon 멀티스레드 병렬 압축 맵 수행 (`par_iter()` 활용)
    // - par_iter(): 일반 iter() 대신 사용 시, Rayon이 내부적으로 작업 훔치기(Work-Stealing) 풀을 사용하여
    //   CPU의 모든 코어로 분산 처리 작업을 가동시킵니다.
    let global_dict_ref = global_dict.as_ref();
    let compressed_chunks: Vec<Result<Vec<u8>, MzcError>> = chunks
        .par_iter()
        .enumerate()
        .map(|(chunk_idx, &chunk)| {
            let start_time = std::time::Instant::now();
            let chunk_orig_size = chunk.len() as u32;

            // 동작 모드 매핑
            let alg_type = match mode {
                CompressionMode::Rle => ALGORITHM_RLE,
                CompressionMode::Dict => ALGORITHM_DICT,
                CompressionMode::Hybrid => ALGORITHM_HYBRID,
                CompressionMode::Lz77 => ALGORITHM_LZ77,
            };

            // A. 전처리 필터 적용
            let mut processed_chunk = chunk.to_vec();
            if is_v7 {
                // MZC7의 경우 미디어 전용 필터를 최우선 적용합니다.
                if png {
                    filters::apply_png_filter(&mut processed_chunk);
                } else if lpc {
                    filters::apply_lpc_filter(&mut processed_chunk);
                } else {
                    // 미디어 필터가 없으면 기존 Delta/BCJ 필터 적용
                    if bcj {
                        rle::apply_bcj_filter(&mut processed_chunk);
                    }
                    if delta {
                        rle::apply_delta_filter(&mut processed_chunk);
                    }
                }
            } else {
                // 구버전 규격 필터 적용
                if bcj {
                    rle::apply_bcj_filter(&mut processed_chunk);
                }
                if delta {
                    rle::apply_delta_filter(&mut processed_chunk);
                }
            }

            // B. 사전 추출 및 매칭
            let dict = if let Some(g_dict) = global_dict_ref {
                g_dict.clone()
            } else if alg_type != ALGORITHM_RLE {
                build_dictionary(&processed_chunk)
            } else {
                Dictionary::new()
            };

            // C. RLE 하이브리드 압축 수행
            let config = rle::CompressionConfig::from_level(level);
            let blocks = rle::compress_to_blocks(&processed_chunk, &dict, alg_type, &config);
            let rle_payload = rle::serialize_blocks_v5(&blocks);

            // D. 사전과 RLE 페이로드 결합
            let combined = if global_dict_ref.is_some() {
                rle_payload
            } else {
                let mut combined = dict.to_bytes();
                combined.extend_from_slice(&rle_payload);
                combined
            };

            let chunk_comb_size = combined.len() as u32;

            if entropy == EntropyMode::Cm {
                println!("compress_bytes_v2_with_progress_dict: combined len = {}, first 15 = {:?}", combined.len(), &combined[..std::cmp::min(15, combined.len())]);
            }

            // E. 엔트로피 코딩 추가 압축 적용
            // - `?` 연산자: Result에서 에러가 발생하면 클로저 밖으로 즉시 에러를 반환시킵니다.
            let final_payload = if entropy == EntropyMode::Huffman {
                huffman_compress(&combined)
            } else if entropy == EntropyMode::Dynamic {
                huffman_compress_dynamic(&combined)
            } else if entropy == EntropyMode::Ans {
                ans::ans_compress(&combined)?
            } else if entropy == EntropyMode::Cm {
                let res = cm::cm_compress(&combined);
                if let Ok(ref comp) = res {
                    println!("compress_bytes_v2_with_progress_dict: CM input len = {}, output len = {}", combined.len(), comp.len());
                    println!("compress_bytes_v2_with_progress_dict: CM input 15 = {:?}", &combined[..std::cmp::min(15, combined.len())]);
                    println!("compress_bytes_v2_with_progress_dict: CM output 15 = {:?}", &comp[..std::cmp::min(15, comp.len())]);
                }
                res?
            } else {
                combined
            };

            let chunk_comp_size = final_payload.len() as u32;

            // 실시간 진행 상황 콜백 발동
            let duration_secs = start_time.elapsed().as_secs_f64();
            on_chunk_progress(chunk_idx, chunk_orig_size as usize, chunk_comp_size as usize, duration_secs);

            // F. 청크 개별 바이너리 생성: [Original u32] [Combined u32] [Compressed u32] + 페이로드
            let mut chunk_bin = Vec::with_capacity(CHUNK_HEADER_SIZE + final_payload.len());
            chunk_bin.extend_from_slice(&chunk_orig_size.to_le_bytes());
            chunk_bin.extend_from_slice(&chunk_comb_size.to_le_bytes());
            chunk_bin.extend_from_slice(&chunk_comp_size.to_le_bytes());
            chunk_bin.extend_from_slice(&final_payload);

            Ok(chunk_bin)
        })
        .collect();

    // 병렬 작업 에러 수집 검사
    let mut payload_buffer = Vec::new();
    for chunk_res in compressed_chunks {
        match chunk_res {
            Ok(chunk) => payload_buffer.extend_from_slice(&chunk),
            Err(_) => return Vec::new(),
        }
    }

    // 원본 전체 SHA-256 해시 계산
    let total_sha256 = calculate_sha256(original);

    // 3. 파일 헤더의 알고리즘 타입 플래그 비트 빌드 (비트 조작 마스크)
    let algorithm_type_flag = if is_v7 {
        // MZC7 비트 패킹 구조 매핑:
        // - bits 0-1: 코어 알고리즘 (0 = Rle, 1 = Dict, 2 = Hybrid, 3 = Lz77)
        let core_bits = match mode {
            CompressionMode::Rle => 0,
            CompressionMode::Dict => 1,
            CompressionMode::Hybrid => 2,
            CompressionMode::Lz77 => 3,
        };
        // - bits 2-4: 엔트로피 모드 (0 = None, 1 = Huffman, 2 = Dynamic, 3 = Ans, 4 = Cm)
        let entropy_bits = match entropy {
            EntropyMode::None => 0,
            EntropyMode::Huffman => 1,
            EntropyMode::Dynamic => 2,
            EntropyMode::Ans => 3,
            EntropyMode::Cm => 4,
        };
        // - bits 5-7: 필터 모드 (0 = None, 1 = Delta, 2 = BCJ, 3 = PNG, 4 = LPC, 5 = Delta + BCJ)
        let filter_bits = if png {
            3
        } else if lpc {
            4
        } else if delta && bcj {
            5
        } else if delta {
            1
        } else if bcj {
            2
        } else {
            0
        };

        // 비트들을 쉬프트 시켜 1개의 바이트 플래그 정보로 패킹합니다.
        core_bits | (entropy_bits << 2) | (filter_bits << 5)
    } else {
        // 기존 버전들 알고리즘 플래그 빌드
        let core_alg = match (mode, entropy) {
            (CompressionMode::Rle, EntropyMode::None) => ALGORITHM_RLE,
            (CompressionMode::Lz77, _) => ALGORITHM_LZ77,
            _ => ALGORITHM_HYBRID,
        };
        core_alg
            | (if delta { FILTER_DELTA } else { 0 })
            | (if bcj { FILTER_BCJ } else { 0 })
            | (if entropy == EntropyMode::Dynamic { FILTER_DYNAMIC_HUFFMAN } else { 0 })
            | (if entropy == EntropyMode::Ans { FILTER_ANS } else { 0 })
    };

    let is_v6 = dictionary_size > 0 || entropy == EntropyMode::Ans;

    // 헤더 구조체 조립
    let header = if is_v7 {
        MzcHeader::new_v7(
            algorithm_type_flag,
            original.len() as u64,
            (payload_buffer.len() + global_dict_bytes.len()) as u64,
            dictionary_size,
            total_sha256,
        )
    } else if is_v6 {
        MzcHeader::new_v6(
            algorithm_type_flag,
            original.len() as u64,
            (payload_buffer.len() + global_dict_bytes.len()) as u64,
            dictionary_size,
            total_sha256,
        )
    } else {
        MzcHeader::new_v5(
            algorithm_type_flag,
            original.len() as u64,
            payload_buffer.len() as u64,
            0,
            total_sha256,
        )
    };

    // 출력 바이너리 조합
    let mut final_output = header.to_bytes();
    if (is_v7 || is_v6) && dictionary_size > 0 {
        final_output.extend_from_slice(&global_dict_bytes);
    }
    final_output.extend_from_slice(&payload_buffer);

    final_output
}

/// **MZC 압축 바이너리 전체를 읽어와 MZC1~MZC7 포맷을 자동 감별하고 멀티스레드로 각 청크를 병렬 해제합니다.**
pub fn decompress_bytes_v2(mzc_bytes: &[u8]) -> Result<Vec<u8>, MzcError> {
    decompress_bytes_v2_dict(mzc_bytes, None)
}

/// **외부 사전 데이터를 지원하여 해제 복원하는 확장 압축 해제 엔트리포인트입니다.**
pub fn decompress_bytes_v2_dict(mzc_bytes: &[u8], dict_data: Option<&[u8]>) -> Result<Vec<u8>, MzcError> {
    if mzc_bytes.len() < 4 {
        return Err(MzcError::TruncatedHeader { read_bytes: mzc_bytes.len() });
    }

    // 1. 헤더 복구
    let header = MzcHeader::from_bytes(mzc_bytes)?;

    // MZC1 복구 (단일 스레드 RLE 기반 하위 호환 구조)
    if header.version == VERSION_MZC1 {
        let payload = &mzc_bytes[HEADER_SIZE_MZC1..];
        if payload.len() != header.payload_size as usize {
            return Err(MzcError::TruncatedBlock {
                expected: header.payload_size as usize,
                found: payload.len(),
            });
        }
        let decompressed = rle::rle_decompress(payload)?;
        
        // 크기 및 체크섬 매칭 검증
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

    // 빈 파일 조기 리턴
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

    // A. 전역 공유 사전 복구
    let global_dict = if let Some(bytes) = dict_data {
        Some(Dictionary::from_bytes(bytes)?)
    } else if header.dictionary_size > 0 {
        let dict_size = header.dictionary_size as usize;
        if payload_area.len() < dict_size {
            return Err(MzcError::CorruptDictionary);
        }
        Some(Dictionary::from_bytes(&payload_area[0..dict_size])?)
    } else {
        None
    };

    // B. 페이로드 영역에서 청크 바이트 슬라이스 추출
    let mut chunk_slices = Vec::new();
    let mut pos = header.dictionary_size as usize;
    let n = payload_area.len();
    let mut total_orig_size = 0u64;

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

        // Safety limit checks to prevent OOM on malformed inputs
        if chunk_orig_size > CHUNK_LIMIT {
            return Err(MzcError::OriginalSizeMismatch {
                expected: CHUNK_LIMIT as u64,
                found: chunk_orig_size as u64,
            });
        }
        if chunk_comb_size > CHUNK_LIMIT * 4 {
            return Err(MzcError::TruncatedBlock {
                expected: CHUNK_LIMIT * 4,
                found: chunk_comb_size,
            });
        }

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
        total_orig_size += chunk_orig_size as u64;
    }

    // Verify parsed total size matches header original size to prevent OOM
    if total_orig_size != header.original_size {
        return Err(MzcError::OriginalSizeMismatch {
            expected: header.original_size,
            found: total_orig_size,
        });
    }

    // C. Rayon 멀티스레드로 각 청크를 동시 병렬 디코딩 (par_iter + map)
    let global_dict_ref = global_dict.as_ref();
    let decompressed_chunks: Result<Vec<Vec<u8>>, MzcError> = chunk_slices
        .par_iter()
        .map(|&(chunk_data, chunk_orig_size, chunk_comb_size)| {
            // 엔트로피 타입 복원 감지 분기
            let (is_huffman, is_dynamic, is_ans, is_cm) = if header.version == VERSION_MZC7 {
                let entropy_bits = (header.algorithm_type >> 2) & 0x07;
                (
                    entropy_bits == 1,
                    entropy_bits == 2,
                    entropy_bits == 3,
                    entropy_bits == 4,
                )
            } else {
                (
                    header.version < VERSION_MZC7 && chunk_data.len() != chunk_comb_size && (header.version != VERSION_MZC4 && (header.version < VERSION_MZC5 || (header.algorithm_type & FILTER_DYNAMIC_HUFFMAN) == 0) && (header.version < VERSION_MZC6 || (header.algorithm_type & FILTER_ANS) == 0)),
                    header.version == VERSION_MZC4 || (header.version >= VERSION_MZC5 && (header.algorithm_type & FILTER_DYNAMIC_HUFFMAN) != 0),
                    header.version >= VERSION_MZC6 && (header.algorithm_type & FILTER_ANS) != 0,
                    false,
                )
            };

            // 엔트로피 압축 해제 실행
            let unhuffman = if is_cm {
                let res = cm::cm_decompress(chunk_data, chunk_comb_size);
                if let Ok(ref decomp) = res {
                    println!("decompress_bytes_v2_dict: CM input len = {}, decompressed len = {}, expected = {}", chunk_data.len(), decomp.len(), chunk_comb_size);
                    println!("decompress_bytes_v2_dict: CM input 15 = {:?}", &chunk_data[..std::cmp::min(15, chunk_data.len())]);
                    println!("decompress_bytes_v2_dict: CM decompressed 15 = {:?}", &decomp[..std::cmp::min(15, decomp.len())]);
                } else if let Err(ref err) = res {
                    println!("decompress_bytes_v2_dict: CM decompress ERROR: {:?}", err);
                }
                res?
            } else if is_ans {
                ans::ans_decompress(chunk_data, chunk_comb_size)?
            } else if is_dynamic {
                huffman_decompress_dynamic(chunk_data, chunk_comb_size)?
            } else if is_huffman {
                huffman_decompress(chunk_data, chunk_comb_size)?
            } else {
                chunk_data.to_vec()
            };

            if unhuffman.len() < 2 && header.dictionary_size == 0 {
                return Ok(unhuffman);
            }

            // 로컬 사전 및 RLE 페이로드 분리
            let (dict, rle_payload) = if header.dictionary_size > 0 {
                let g_dict = global_dict_ref.cloned().unwrap_or_default();
                (g_dict, unhuffman)
            } else {
                let dict = Dictionary::from_bytes(&unhuffman)?;
                let dict_bytes_len = dict.to_bytes().len();
                if dict_bytes_len > unhuffman.len() {
                    return Err(MzcError::CorruptDictionary);
                }
                let payload = unhuffman[dict_bytes_len..].to_vec();
                (dict, payload)
            };

            // 디코딩용 코어 알고리즘 형태 추출
            let core_alg = if header.version == VERSION_MZC7 {
                let core_bits = header.algorithm_type & 0x03;
                match core_bits {
                    0 => ALGORITHM_RLE,
                    1 => ALGORITHM_DICT,
                    2 => ALGORITHM_HYBRID,
                    3 => ALGORITHM_LZ77,
                    _ => unreachable!(),
                }
            } else if header.version >= VERSION_MZC5 {
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

            // RLE / LZ77 블록 압축 해제 복원
            let mut decompressed_chunk = if header.version >= VERSION_MZC5 {
                rle_decompress_hybrid_mzc5(&rle_payload, &dict, alg_flag, chunk_orig_size)?
            } else {
                rle_decompress_hybrid(&rle_payload, &dict, alg_flag, chunk_orig_size)?
            };

            // 역전처리 필터 적용 (인코딩의 역순)
            if header.version == VERSION_MZC7 {
                let filter_bits = (header.algorithm_type >> 5) & 0x07;
                match filter_bits {
                    1 => {
                        rle::inverse_delta_filter(&mut decompressed_chunk);
                    }
                    2 => {
                        rle::inverse_bcj_filter(&mut decompressed_chunk);
                    }
                    3 => {
                        filters::inverse_png_filter(&mut decompressed_chunk);
                    }
                    4 => {
                        filters::inverse_lpc_filter(&mut decompressed_chunk);
                    }
                    5 => {
                        // Delta + BCJ 적용된 역연산
                        rle::inverse_delta_filter(&mut decompressed_chunk);
                        rle::inverse_bcj_filter(&mut decompressed_chunk);
                    }
                    _ => {}
                }
            } else if header.version >= VERSION_MZC5 {
                let has_delta = (header.algorithm_type & FILTER_DELTA) != 0;
                let has_bcj = (header.algorithm_type & FILTER_BCJ) != 0;
                if has_delta {
                    rle::inverse_delta_filter(&mut decompressed_chunk);
                }
                if has_bcj {
                    rle::inverse_bcj_filter(&mut decompressed_chunk);
                }
            }

            // 개별 청크 원본 크기 교차 검증
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

    // D. 복원 완료된 전체 청크 데이터 병합
    let mut restored_bytes = Vec::with_capacity(header.original_size as usize);
    for chunk in decompressed_chunks {
        restored_bytes.extend_from_slice(&chunk);
    }

    // E. 최종 체크섬 및 원래 크기 재차 검증
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
