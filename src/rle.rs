use std::collections::HashMap;
use crate::error::MzcError;
use crate::format::{ALGORITHM_RLE, ALGORITHM_DICT, ALGORITHM_HYBRID, ALGORITHM_LZ77};

const BLOCK_TYPE_LITERAL: u8 = 0x00;
const BLOCK_TYPE_RUN: u8 = 0x01;
const BLOCK_TYPE_TOKEN: u8 = 0x02;
const BLOCK_TYPE_BACKREF: u8 = 0x03;

const LZ77_WINDOW_SIZE: usize = 32768;
const MAX_BLOCK_LEN: usize = 65535;

/// MZC2 사전 섹션을 나타내는 구조체입니다.
/// 바이트 슬라이스(`Vec<u8>`)를 엔트리로 지녀 텍스트와 원시 바이너리를 모두 지원합니다.
#[derive(Debug, Clone, Default)]
pub struct Dictionary {
    pub entries: Vec<Vec<u8>>,
}

impl Dictionary {
    /// 새로운 빈 사전을 생성합니다.
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// 사전을 2바이트 카운트와 가변 엔트리 구조로 직렬화합니다.
    ///
    /// # 직렬화 명세:
    /// `[Entry Count: 2 bytes (u16 le)]` + `[Entry Length: 1 byte (u8)]` + `[Entry Data: N bytes]` 반복
    pub fn to_bytes(&self) -> Vec<u8> {
        let count = self.entries.len() as u16;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&count.to_le_bytes());

        for entry in &self.entries {
            // 각 엔트리의 길이는 최대 255바이트로 제약됩니다.
            let len = entry.len() as u8;
            bytes.push(len);
            bytes.extend_from_slice(entry);
        }
        bytes
    }

    /// 이진 데이터 스트림으로부터 사전을 파싱하여 복구합니다.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MzcError> {
        if bytes.len() < 2 {
            return Err(MzcError::CorruptDictionary);
        }

        let count_bytes: [u8; 2] = bytes[0..2]
            .try_into()
            .map_err(|_| MzcError::CorruptDictionary)?;
        let count = u16::from_le_bytes(count_bytes) as usize;

        let mut entries = Vec::with_capacity(count);
        let mut pos = 2;

        for _ in 0..count {
            if pos >= bytes.len() {
                return Err(MzcError::CorruptDictionary);
            }
            let len = bytes[pos] as usize;
            pos += 1;

            if pos + len > bytes.len() {
                return Err(MzcError::CorruptDictionary);
            }
            let entry = bytes[pos..pos + len].to_vec();
            entries.push(entry);
            pos += len;
        }

        Ok(Self { entries })
    }
}

/// 입력 데이터 스트림에서 최적의 사전 엔트리들을 추천 추출합니다.
///
/// # 스코어링 가중치 공식:
/// `Score = (L - 3) * F - (2 + L)`
/// - $L$ (패턴 크기, 4 ~ 16바이트)
/// - $F$ (출현 빈도수)
pub fn build_dictionary(data: &[u8]) -> Dictionary {
    if data.len() < 10 {
        return Dictionary::default();
    }

    let mut pattern_counts = HashMap::new();
    let n = data.len();

    // 1. 슬라이딩 윈도우 스캔 (패턴 크기 L: 4 ~ 16바이트 범위 조사)
    for len in 4..=16 {
        if len > n {
            break;
        }
        for i in 0..=(n - len) {
            let pattern = &data[i..i + len];
            *pattern_counts.entry(pattern.to_vec()).or_insert(0usize) += 1;
        }
    }

    // 2. 이득 평가 산출 및 스코어링
    let mut candidates = Vec::new();
    for (pattern, freq) in pattern_counts {
        if freq < 2 {
            continue; // 최소 2번 이상은 중복 출현해야 사전적 이득이 발생합니다.
        }
        let l = pattern.len();
        let score = ((l as isize - 3) * freq as isize) - (2 + l as isize);
        if score > 0 {
            candidates.push((pattern, score));
        }
    }

    // 3. 고득점 순으로 정렬
    candidates.sort_by(|a, b| b.1.cmp(&a.1));

    // 실무 최적화 성능과 빠른 인코딩을 위해 최대 사전 엔트리 개수를 256개로 제한합니다.
    let limit = std::cmp::min(candidates.len(), 256);
    let mut entries = Vec::with_capacity(limit);
    for i in 0..limit {
        entries.push(candidates[i].0.clone());
    }

    Dictionary { entries }
}

#[derive(Debug, Clone)]
pub enum CompressBlock {
    Literal(Vec<u8>),
    Run { count: u16, value: u8 },
    Token(u16),
    BackRef { distance: u16, length: u16 },
}

#[derive(Debug, Clone, Copy)]
pub struct CompressionConfig {
    pub window_size: usize,
    pub scan_limit: usize,
    pub lazy_matching: bool,
}

impl CompressionConfig {
    pub fn from_level(level: u8) -> Self {
        match level {
            1 => Self { window_size: 1024, scan_limit: 32, lazy_matching: false },
            2 => Self { window_size: 2048, scan_limit: 64, lazy_matching: false },
            3 => Self { window_size: 4096, scan_limit: 128, lazy_matching: false },
            4 => Self { window_size: 8192, scan_limit: 256, lazy_matching: true },
            5 => Self { window_size: 16384, scan_limit: 512, lazy_matching: true },
            6 => Self { window_size: 32768, scan_limit: 2048, lazy_matching: true },
            7 => Self { window_size: 32768, scan_limit: 4096, lazy_matching: true },
            8 => Self { window_size: 65536, scan_limit: 8192, lazy_matching: true },
            9 => Self { window_size: 65536, scan_limit: 32768, lazy_matching: true },
            _ => Self { window_size: 32768, scan_limit: 2048, lazy_matching: true },
        }
    }
}

pub fn apply_delta_filter(data: &mut [u8]) {
    if data.is_empty() { return; }
    for i in (1..data.len()).rev() {
        data[i] = data[i].wrapping_sub(data[i - 1]);
    }
}

pub fn inverse_delta_filter(data: &mut [u8]) {
    if data.is_empty() { return; }
    for i in 1..data.len() {
        data[i] = data[i].wrapping_add(data[i - 1]);
    }
}

pub fn apply_bcj_filter(data: &mut [u8]) {
    let mut i = 0;
    if data.len() < 5 {
        return;
    }
    while i + 4 < data.len() {
        let op = data[i];
        if op == 0xE8 || op == 0xE9 {
            let offset_bytes: [u8; 4] = data[i + 1..i + 5].try_into().unwrap();
            let rel = u32::from_le_bytes(offset_bytes);
            let abs = rel.wrapping_add(i as u32 + 5);
            data[i + 1..i + 5].copy_from_slice(&abs.to_le_bytes());
            i += 5;
        } else {
            i += 1;
        }
    }
}

pub fn inverse_bcj_filter(data: &mut [u8]) {
    let mut i = 0;
    if data.len() < 5 {
        return;
    }
    while i + 4 < data.len() {
        let op = data[i];
        if op == 0xE8 || op == 0xE9 {
            let abs_bytes: [u8; 4] = data[i + 1..i + 5].try_into().unwrap();
            let abs = u32::from_le_bytes(abs_bytes);
            let rel = abs.wrapping_sub(i as u32 + 5);
            data[i + 1..i + 5].copy_from_slice(&rel.to_le_bytes());
            i += 5;
        } else {
            i += 1;
        }
    }
}

pub fn find_lz77_match_with_limit(
    data: &[u8],
    pos: usize,
    window_size: usize,
    scan_limit: usize,
) -> Option<(u16, u16)> {
    if pos == 0 {
        return None;
    }

    let start = if pos > window_size { pos - window_size } else { 0 };
    let mut best_dist = 0;
    let mut best_len = 0;
    let mut steps = 0;

    for j in (start..pos).rev() {
        steps += 1;
        if steps > scan_limit {
            break;
        }

        if data[j] != data[pos] {
            continue;
        }
        if best_len > 0 && j + best_len < pos && pos + best_len < data.len() && data[j + best_len] != data[pos + best_len] {
            continue;
        }

        let max_possible = std::cmp::min(data.len() - pos, MAX_BLOCK_LEN);
        let mut len = 0;
        while len < max_possible && data[j + len] == data[pos + len] {
            len += 1;
        }

        if len > best_len {
            best_len = len;
            best_dist = pos - j;
        }
    }

    if best_len >= 4 {
        Some((best_dist as u16, best_len as u16))
    } else {
        None
    }
}

pub fn find_lz77_match(data: &[u8], pos: usize, window_size: usize) -> Option<(u16, u16)> {
    find_lz77_match_with_limit(data, pos, window_size, 4096)
}

pub fn compress_to_blocks(
    data: &[u8],
    dict: &Dictionary,
    algorithm_type: u8,
    config: &CompressionConfig,
) -> Vec<CompressBlock> {
    let mut blocks = Vec::new();
    let mut literal_buffer = Vec::new();
    let n = data.len();
    let mut i = 0;

    let mut flush_literal = |lit_buf: &mut Vec<u8>, blks: &mut Vec<CompressBlock>| {
        if lit_buf.is_empty() {
            return;
        }
        blks.push(CompressBlock::Literal(lit_buf.clone()));
        lit_buf.clear();
    };

    while i < n {
        let mut run_savings = -9999isize;
        let mut token_savings = -9999isize;
        let mut lz77_savings = -9999isize;

        // 1. RLE run
        let mut run_count = 0;
        if algorithm_type == ALGORITHM_RLE || algorithm_type == ALGORITHM_HYBRID || algorithm_type == ALGORITHM_LZ77 {
            let current_val = data[i];
            while i + run_count < n && data[i + run_count] == current_val && run_count < MAX_BLOCK_LEN {
                run_count += 1;
            }
            if run_count >= 4 {
                run_savings = run_count as isize - 4;
            }
        }

        // 2. Token match
        let mut best_match_idx: Option<usize> = None;
        let mut best_match_len = 0;
        if algorithm_type == ALGORITHM_DICT || algorithm_type == ALGORITHM_HYBRID || algorithm_type == ALGORITHM_LZ77 {
            for (idx, entry) in dict.entries.iter().enumerate() {
                let entry_len = entry.len();
                if i + entry_len <= n && &data[i..i + entry_len] == entry {
                    if entry_len > best_match_len {
                        best_match_idx = Some(idx);
                        best_match_len = entry_len;
                    }
                }
            }
            if best_match_len >= 4 {
                token_savings = best_match_len as isize - 3;
            }
        }

        // 3. LZ77 match
        let mut lz77_dist = 0u16;
        let mut lz77_len = 0u16;
        if algorithm_type == ALGORITHM_LZ77 {
            if let Some((dist, len)) = find_lz77_match_with_limit(data, i, config.window_size, config.scan_limit) {
                let mut defer_match = false;
                if config.lazy_matching && i + 1 < n {
                    if let Some((_, next_len)) = find_lz77_match_with_limit(data, i + 1, config.window_size, config.scan_limit) {
                        if next_len > len {
                            defer_match = true;
                        }
                    }
                }

                if defer_match {
                    lz77_savings = -9999;
                } else {
                    lz77_dist = dist;
                    lz77_len = len;
                    lz77_savings = len as isize - 5;
                }
            }
        }

        let max_savings = std::cmp::max(run_savings, std::cmp::max(token_savings, lz77_savings));

        if max_savings >= 0 {
            flush_literal(&mut literal_buffer, &mut blocks);
            if max_savings == token_savings && best_match_idx.is_some() {
                let token_idx = best_match_idx.unwrap() as u16;
                blocks.push(CompressBlock::Token(token_idx));
                i += best_match_len;
            } else if max_savings == run_savings {
                blocks.push(CompressBlock::Run { count: run_count as u16, value: data[i] });
                i += run_count;
            } else {
                blocks.push(CompressBlock::BackRef { distance: lz77_dist, length: lz77_len });
                i += lz77_len as usize;
            }
        } else {
            literal_buffer.push(data[i]);
            i += 1;
            if literal_buffer.len() == MAX_BLOCK_LEN {
                flush_literal(&mut literal_buffer, &mut blocks);
            }
        }
    }

    flush_literal(&mut literal_buffer, &mut blocks);
    blocks
}

pub fn serialize_blocks_v2(blocks: &[CompressBlock]) -> Vec<u8> {
    let mut out = Vec::new();
    for block in blocks {
        match block {
            CompressBlock::Literal(lit) => {
                out.push(BLOCK_TYPE_LITERAL);
                let len = lit.len() as u16;
                out.extend_from_slice(&len.to_le_bytes());
                out.extend_from_slice(lit);
            }
            CompressBlock::Run { count, value } => {
                out.push(BLOCK_TYPE_RUN);
                out.extend_from_slice(&count.to_le_bytes());
                out.push(*value);
            }
            CompressBlock::Token(idx) => {
                out.push(BLOCK_TYPE_TOKEN);
                out.extend_from_slice(&idx.to_le_bytes());
            }
            CompressBlock::BackRef { distance, length } => {
                out.push(BLOCK_TYPE_BACKREF);
                out.extend_from_slice(&distance.to_le_bytes());
                out.extend_from_slice(&length.to_le_bytes());
            }
        }
    }
    out
}

pub fn serialize_blocks_v5(blocks: &[CompressBlock]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    let n = blocks.len();

    while i < n {
        let chunk_blocks = &blocks[i..std::cmp::min(i + 8, n)];
        let mut flag: u16 = 0;
        let flag_pos = out.len();
        out.push(0);
        out.push(0);

        for (k, block) in chunk_blocks.iter().enumerate() {
            let block_type = match block {
                CompressBlock::Literal(_) => 0,
                CompressBlock::Run { .. } => 1,
                CompressBlock::Token(_) => 2,
                CompressBlock::BackRef { .. } => 3,
            };
            flag |= (block_type as u16) << (2 * k);

            match block {
                CompressBlock::Literal(lit) => {
                    let len = lit.len() as u16;
                    out.extend_from_slice(&len.to_le_bytes());
                    out.extend_from_slice(lit);
                }
                CompressBlock::Run { count, value } => {
                    out.extend_from_slice(&count.to_le_bytes());
                    out.push(*value);
                }
                CompressBlock::Token(idx) => {
                    out.extend_from_slice(&idx.to_le_bytes());
                }
                CompressBlock::BackRef { distance, length } => {
                    out.extend_from_slice(&distance.to_le_bytes());
                    out.extend_from_slice(&length.to_le_bytes());
                }
            }
        }
        
        let flag_bytes = flag.to_le_bytes();
        out[flag_pos] = flag_bytes[0];
        out[flag_pos + 1] = flag_bytes[1];

        i += 8;
    }

    out
}

/// RLE, 사전 토큰, LZ77 백레퍼런스, 리터럴 블록을 동작 모드(`algorithm_type`)에 맞춰 탐욕적으로 인코딩합니다.
pub fn rle_compress_hybrid(
    data: &[u8],
    dict: &Dictionary,
    algorithm_type: u8,
) -> Vec<u8> {
    let config = CompressionConfig::from_level(6);
    let blocks = compress_to_blocks(data, dict, algorithm_type, &config);
    serialize_blocks_v2(&blocks)
}

/// MZC2 하이브리드 블록 스트림 페이로드를 읽어와 원래 바이트 데이터로 해제 복원합니다.
pub fn rle_decompress_hybrid(
    payload: &[u8],
    dict: &Dictionary,
    algorithm_type: u8,
    max_size: usize,
) -> Result<Vec<u8>, MzcError> {
    let mut decompressed = Vec::new();
    let mut pos = 0;
    let n = payload.len();

    while pos < n {
        if pos + 3 > n {
            return Err(MzcError::TruncatedBlock {
                expected: 3,
                found: n - pos,
            });
        }

        let block_type = payload[pos];
        let len_bytes: [u8; 2] = payload[pos + 1..pos + 3]
            .try_into()
            .expect("블록 크기 u16 변환");
        let block_len = u16::from_le_bytes(len_bytes) as usize;
        pos += 3;

        match block_type {
            BLOCK_TYPE_LITERAL => {
                if decompressed.len() + block_len > max_size {
                    return Err(MzcError::OriginalSizeMismatch {
                        expected: max_size as u64,
                        found: (decompressed.len() + block_len) as u64,
                    });
                }
                if pos + block_len > n {
                    return Err(MzcError::TruncatedBlock {
                        expected: block_len,
                        found: n - pos,
                    });
                }
                decompressed.extend_from_slice(&payload[pos..pos + block_len]);
                pos += block_len;
            }
            BLOCK_TYPE_RUN => {
                if algorithm_type == ALGORITHM_DICT {
                    return Err(MzcError::InvalidAlgorithm {
                        expected: ALGORITHM_HYBRID,
                        found: ALGORITHM_DICT,
                    });
                }
                if decompressed.len() + block_len > max_size {
                    return Err(MzcError::OriginalSizeMismatch {
                        expected: max_size as u64,
                        found: (decompressed.len() + block_len) as u64,
                    });
                }

                if pos + 1 > n {
                    return Err(MzcError::TruncatedBlock {
                        expected: 1,
                        found: n - pos,
                    });
                }
                let value = payload[pos];
                pos += 1;
                decompressed.resize(decompressed.len() + block_len, value);
            }
            BLOCK_TYPE_TOKEN => {
                if algorithm_type == ALGORITHM_RLE {
                    return Err(MzcError::InvalidAlgorithm {
                        expected: ALGORITHM_HYBRID,
                        found: ALGORITHM_RLE,
                    });
                }

                let token_idx = block_len;
                if token_idx >= dict.entries.len() {
                    return Err(MzcError::InvalidTokenIndex {
                        index: token_idx as u16,
                        max_valid: dict.entries.len() as u16,
                    });
                }

                let entry = &dict.entries[token_idx];
                if decompressed.len() + entry.len() > max_size {
                    return Err(MzcError::OriginalSizeMismatch {
                        expected: max_size as u64,
                        found: (decompressed.len() + entry.len()) as u64,
                    });
                }
                decompressed.extend_from_slice(entry);
            }
            BLOCK_TYPE_BACKREF => {
                if algorithm_type == ALGORITHM_RLE || algorithm_type == ALGORITHM_DICT {
                    return Err(MzcError::InvalidAlgorithm {
                        expected: ALGORITHM_LZ77,
                        found: algorithm_type,
                    });
                }

                if pos + 2 > n {
                    return Err(MzcError::TruncatedBlock {
                        expected: 2,
                        found: n - pos,
                    });
                }

                let dist = block_len;
                let len_bytes: [u8; 2] = payload[pos..pos + 2].try_into().unwrap();
                let length = u16::from_le_bytes(len_bytes) as usize;
                pos += 2;

                if decompressed.len() + length > max_size {
                    return Err(MzcError::OriginalSizeMismatch {
                        expected: max_size as u64,
                        found: (decompressed.len() + length) as u64,
                    });
                }

                let current_size = decompressed.len();
                if dist == 0 || dist > current_size {
                    return Err(MzcError::InvalidBackRef {
                        distance: dist as u16,
                        length: length as u16,
                        current_size,
                    });
                }

                let start_idx = current_size - dist;
                for offset in 0..length {
                    let val = decompressed[start_idx + offset];
                    decompressed.push(val);
                }
            }
            _ => {
                return Err(MzcError::UnknownBlockType { found: block_type });
            }
        }
    }

    Ok(decompressed)
}

pub fn rle_decompress_hybrid_mzc5(
    payload: &[u8],
    dict: &Dictionary,
    algorithm_type: u8,
    chunk_orig_size: usize,
) -> Result<Vec<u8>, MzcError> {
    let mut decompressed = Vec::new();
    let mut pos = 0;
    let n = payload.len();

    while decompressed.len() < chunk_orig_size {
        if pos + 2 > n {
            return Err(MzcError::TruncatedBlock {
                expected: 2,
                found: n - pos,
            });
        }

        let flag_bytes: [u8; 2] = payload[pos..pos + 2].try_into().unwrap();
        let flag = u16::from_le_bytes(flag_bytes);
        pos += 2;

        for k in 0..8 {
            if decompressed.len() >= chunk_orig_size {
                break;
            }

            let block_type = ((flag >> (2 * k)) & 0x03) as u8;
            match block_type {
                BLOCK_TYPE_LITERAL => {
                    if pos + 2 > n {
                        return Err(MzcError::TruncatedBlock {
                            expected: 2,
                            found: n - pos,
                        });
                    }
                    let len_bytes: [u8; 2] = payload[pos..pos + 2].try_into().unwrap();
                    let block_len = u16::from_le_bytes(len_bytes) as usize;
                    pos += 2;

                    if decompressed.len() + block_len > chunk_orig_size {
                        return Err(MzcError::OriginalSizeMismatch {
                            expected: chunk_orig_size as u64,
                            found: (decompressed.len() + block_len) as u64,
                        });
                    }

                    if pos + block_len > n {
                        return Err(MzcError::TruncatedBlock {
                            expected: block_len,
                            found: n - pos,
                        });
                    }
                    decompressed.extend_from_slice(&payload[pos..pos + block_len]);
                    pos += block_len;
                }
                BLOCK_TYPE_RUN => {
                    if algorithm_type == ALGORITHM_DICT {
                        return Err(MzcError::InvalidAlgorithm {
                            expected: ALGORITHM_HYBRID,
                            found: ALGORITHM_DICT,
                        });
                    }

                    if pos + 3 > n {
                        return Err(MzcError::TruncatedBlock {
                            expected: 3,
                            found: n - pos,
                        });
                    }
                    let count_bytes: [u8; 2] = payload[pos..pos + 2].try_into().unwrap();
                    let count = u16::from_le_bytes(count_bytes) as usize;
                    let value = payload[pos + 2];
                    pos += 3;

                    if decompressed.len() + count > chunk_orig_size {
                        return Err(MzcError::OriginalSizeMismatch {
                            expected: chunk_orig_size as u64,
                            found: (decompressed.len() + count) as u64,
                        });
                    }

                    decompressed.resize(decompressed.len() + count, value);
                }
                BLOCK_TYPE_TOKEN => {
                    if algorithm_type == ALGORITHM_RLE {
                        return Err(MzcError::InvalidAlgorithm {
                            expected: ALGORITHM_HYBRID,
                            found: ALGORITHM_RLE,
                        });
                    }

                    if pos + 2 > n {
                        return Err(MzcError::TruncatedBlock {
                            expected: 2,
                            found: n - pos,
                        });
                    }
                    let idx_bytes: [u8; 2] = payload[pos..pos + 2].try_into().unwrap();
                    let token_idx = u16::from_le_bytes(idx_bytes) as usize;
                    pos += 2;

                    if token_idx >= dict.entries.len() {
                        return Err(MzcError::InvalidTokenIndex {
                            index: token_idx as u16,
                            max_valid: dict.entries.len() as u16,
                        });
                    }

                    let entry = &dict.entries[token_idx];
                    if decompressed.len() + entry.len() > chunk_orig_size {
                        return Err(MzcError::OriginalSizeMismatch {
                            expected: chunk_orig_size as u64,
                            found: (decompressed.len() + entry.len()) as u64,
                        });
                    }
                    decompressed.extend_from_slice(entry);
                }
                BLOCK_TYPE_BACKREF => {
                    if algorithm_type == ALGORITHM_RLE || algorithm_type == ALGORITHM_DICT {
                        return Err(MzcError::InvalidAlgorithm {
                            expected: ALGORITHM_LZ77,
                            found: algorithm_type,
                        });
                    }

                    if pos + 4 > n {
                        return Err(MzcError::TruncatedBlock {
                            expected: 4,
                            found: n - pos,
                        });
                    }

                    let dist_bytes: [u8; 2] = payload[pos..pos + 2].try_into().unwrap();
                    let dist = u16::from_le_bytes(dist_bytes) as usize;
                    let len_bytes: [u8; 2] = payload[pos + 2..pos + 4].try_into().unwrap();
                    let length = u16::from_le_bytes(len_bytes) as usize;
                    pos += 4;

                    if decompressed.len() + length > chunk_orig_size {
                        return Err(MzcError::OriginalSizeMismatch {
                            expected: chunk_orig_size as u64,
                            found: (decompressed.len() + length) as u64,
                        });
                    }

                    let current_size = decompressed.len();
                    if dist == 0 || dist > current_size {
                        return Err(MzcError::InvalidBackRef {
                            distance: dist as u16,
                            length: length as u16,
                            current_size,
                        });
                    }

                    let start_idx = current_size - dist;
                    for offset in 0..length {
                        let val = decompressed[start_idx + offset];
                        decompressed.push(val);
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    Ok(decompressed)
}

/// MZC1 포맷의 압축 페이로드 바이트 슬라이스를 압축 해제하여 원본 바이트 배열로 복원합니다. (하위 호환용)
pub fn rle_decompress(payload: &[u8]) -> Result<Vec<u8>, MzcError> {
    let mut decompressed = Vec::new();
    let mut pos = 0;
    let n = payload.len();

    while pos < n {
        if pos + 3 > n {
            return Err(MzcError::TruncatedBlock {
                expected: 3,
                found: n - pos,
            });
        }

        let block_type = payload[pos];
        let len_bytes: [u8; 2] = payload[pos + 1..pos + 3]
            .try_into()
            .expect("u16 파싱용 2바이트 슬라이스 변환은 항상 성공해야 합니다.");
        let block_len = u16::from_le_bytes(len_bytes) as usize;
        pos += 3;

        match block_type {
            BLOCK_TYPE_LITERAL => {
                if pos + block_len > n {
                    return Err(MzcError::TruncatedBlock {
                        expected: block_len,
                        found: n - pos,
                    });
                }
                decompressed.extend_from_slice(&payload[pos..pos + block_len]);
                pos += block_len;
            }
            BLOCK_TYPE_RUN => {
                if pos + 1 > n {
                    return Err(MzcError::TruncatedBlock {
                        expected: 1,
                        found: n - pos,
                    });
                }
                let value = payload[pos];
                pos += 1;
                decompressed.resize(decompressed.len() + block_len, value);
            }
            _ => {
                return Err(MzcError::UnknownBlockType { found: block_type });
            }
        }
    }

    Ok(decompressed)
}

