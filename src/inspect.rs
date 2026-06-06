use crate::checksum::{bytes_to_hex, calculate_sha256};
use crate::decompress_bytes_v2;
use crate::format::{
    MzcHeader, ALGORITHM_DICT, ALGORITHM_HYBRID, ALGORITHM_LZ77, ALGORITHM_RLE, HEADER_SIZE_MZC1,
    HEADER_SIZE_MZC2, HEADER_SIZE_MZC9, VERSION_MZC1, VERSION_MZC2, VERSION_MZC9,
};
use crate::huffman::huffman_decompress;
use crate::rle::Dictionary;
use anyhow::{Context, Result};
use std::path::Path;

/// MZC 압축 파일을 분석하여 포맷 버전, 헤더 상세, 압축 모드, 실측 압축 통계, SHA-256 무결성 검증 결과를
/// 출력하며, 페이로드 내부의 물리적 블록 배치율을 ANSI 터미널 그래픽 맵으로 시각화 드로잉해 줍니다.
pub fn inspect_mzc_file<P: AsRef<Path>>(file_path: P) -> Result<()> {
    let path = file_path.as_ref();

    // 1. 파일 바이트 로드
    let file_bytes = std::fs::read(path)
        .with_context(|| format!("MZC 파일 '{:?}'을 읽을 수 없습니다.", path))?;

    if file_bytes.len() < 4 {
        anyhow::bail!("파일 크기가 너무 작아 MZC 파일 형식을 분석할 수 없습니다.");
    }

    // 2. 이중 분기 헤더 파싱
    let header = MzcHeader::from_bytes(&file_bytes)
        .context("MZC 헤더 분석에 실패했습니다. 포맷 오염이 의심됩니다.")?;

    let header_size = if header.version == VERSION_MZC9 {
        HEADER_SIZE_MZC9
    } else if header.version >= VERSION_MZC2 {
        HEADER_SIZE_MZC2
    } else {
        HEADER_SIZE_MZC1
    };

    let payload_bytes = &file_bytes[header_size..];
    if payload_bytes.len() != header.payload_size as usize {
        anyhow::bail!(
            "파일 손상 감지: 헤더의 Payload Size({} bytes)와 실제 페이로드 크기({} bytes)가 불일치합니다.",
            header.payload_size,
            payload_bytes.len()
        );
    }

    // 3. 라이브러리 통합 decompress_bytes_v2 파이프라인으로 무결성 및 복원 자체 검증
    let decompressed = decompress_bytes_v2(&file_bytes)
        .context("이진 데이터 압축 해제 및 무결성(Integrity) 검증에 실패했습니다.")?;

    let computed_hash = calculate_sha256(&decompressed);
    let original_hash_str = bytes_to_hex(&header.original_sha256);
    let computed_hash_str = bytes_to_hex(&computed_hash);

    let verified_status = if computed_hash == header.original_sha256 {
        "OK"
    } else {
        "FAILED (SHA-256 Mismatch)"
    };

    // 4. 통계 벤치마크 계산
    let total_compressed_size = file_bytes.len();
    let original_size = header.original_size;
    let ratio = if original_size > 0 {
        (total_compressed_size as f64 / original_size as f64) * 100.0
    } else {
        100.0
    };

    let format_str = if header.version == VERSION_MZC9 {
        "MZC9 (Minimal Zip Concept v9 - Configurable Chunks & Solid)"
    } else if header.version >= VERSION_MZC2 {
        "MZC2-MZC8 Chunk/Parallel Spec"
    } else {
        "MZC1 (Minimal Zip Concept v1 - Single RLE Spec)"
    };

    let mode_str = match header.algorithm_type {
        ALGORITHM_RLE => "RLE Only (Run-Length Encoding)",
        ALGORITHM_DICT => "Dictionary Only (Entropy Enabled)",
        ALGORITHM_HYBRID => "Hybrid Mode (RLE + Dictionary + Static Huffman)",
        ALGORITHM_LZ77 => "LZ77 Hybrid Mode (Runs + Dictionary + BackRefs + Huffman)",
        _ => "Unknown Mode",
    };

    // 5. 결과 기본 정보 출력
    println!("===============================================================================");
    println!(" File: {:?}", path.file_name().unwrap_or(path.as_os_str()));
    println!(" Format: {}", format_str);
    println!(" Algorithm: {}", mode_str);
    println!(" Original size: {} bytes", original_size);
    println!(" Compressed size: {} bytes", total_compressed_size);
    println!(" Ratio: {:.2}%", ratio);
    println!(" SHA-256: {}", original_hash_str);
    if verified_status != "OK" {
        println!(" Computed SHA-256: {}", computed_hash_str);
    }
    println!(" Verified: {}", verified_status);
    println!("===============================================================================");

    // 6. 페이로드 블록 분석 및 ANSI 컬러 비주얼 맵 렌더링
    // 페이로드 영역의 가변 청크 내부 블록(Literal: L, Run: R, Token: T, BackRef: B)들의 분포를 스캔해 수집합니다.
    let mut literal_blocks = 0;
    let mut run_blocks = 0;
    let mut token_blocks = 0;
    let mut backref_blocks = 0;
    let mut visual_blocks = Vec::new();

    if header.version >= VERSION_MZC2 && original_size > 0 {
        // MZC2/MZC3의 청크 세그먼트들을 디코딩하여 스캔 진행
        let mut pos = 0;
        let n = payload_bytes.len();
        while pos < n {
            if pos + 12 > n {
                break;
            }
            let comb_size =
                u32::from_le_bytes(payload_bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
            let comp_size =
                u32::from_le_bytes(payload_bytes[pos + 8..pos + 12].try_into().unwrap()) as usize;
            pos += 12;
            if pos + comp_size > n {
                break;
            }

            let chunk_data = &payload_bytes[pos..pos + comp_size];
            pos += comp_size;

            // 허프만 풀기
            let unhuff = if chunk_data.len() != comb_size {
                huffman_decompress(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
            } else {
                chunk_data.to_vec()
            };

            // 사전 복구
            let dict = Dictionary::from_bytes(&unhuff).unwrap_or_default();
            let dict_bytes_len = dict.to_bytes().len();
            if dict_bytes_len >= unhuff.len() {
                continue;
            }
            let rle_payload = &unhuff[dict_bytes_len..];

            // 블록 해독 및 가상 수집
            let mut b_pos = 0;
            let b_n = rle_payload.len();
            while b_pos < b_n {
                if b_pos + 3 > b_n {
                    break;
                }
                let b_type = rle_payload[b_pos];
                let b_len =
                    u16::from_le_bytes(rle_payload[b_pos + 1..b_pos + 3].try_into().unwrap())
                        as usize;
                b_pos += 3;

                match b_type {
                    0x00 => {
                        literal_blocks += 1;
                        visual_blocks.push('L');
                        b_pos += b_len;
                    }
                    0x01 => {
                        run_blocks += 1;
                        visual_blocks.push('R');
                        b_pos += 1;
                    }
                    0x02 => {
                        token_blocks += 1;
                        visual_blocks.push('T');
                    }
                    0x03 => {
                        backref_blocks += 1;
                        visual_blocks.push('B');
                        b_pos += 2; // BackRef는 dist(2B, b_len) 외에 extra length 2바이트가 뒤따릅니다.
                    }
                    _ => break,
                }
            }
        }
    } else if header.version == VERSION_MZC1 && original_size > 0 {
        // MZC1의 경우 다이렉트로 RLE 블록 스캔 진행
        let mut b_pos = 0;
        let b_n = payload_bytes.len();
        while b_pos < b_n {
            if b_pos + 3 > b_n {
                break;
            }
            let b_type = payload_bytes[b_pos];
            let b_len = u16::from_le_bytes(payload_bytes[b_pos + 1..b_pos + 3].try_into().unwrap())
                as usize;
            b_pos += 3;

            match b_type {
                0x00 => {
                    literal_blocks += 1;
                    visual_blocks.push('L');
                    b_pos += b_len;
                }
                0x01 => {
                    run_blocks += 1;
                    visual_blocks.push('R');
                    b_pos += 1;
                }
                _ => break,
            }
        }
    }

    // 7. 터미널 ANSI 컬러 블록 인쇄
    if !visual_blocks.is_empty() {
        println!(" [ 페이로드 압축 블록 시각화 맵 - Payload Compression Block Map ]");
        println!(" - R (Run, 초록색): 동일바이트 중복 반복");
        println!(" - T (Token, 파란색): 사전식 치환 토큰");
        println!(" - B (BackRef, 노란색): LZ77 슬라이딩 윈도우 백레퍼런스");
        println!(" - L (Literal, 회색): 비압축 원본 바이트");
        println!(" -------------------------------------------------------------");

        let cols = 30; // 가로 출력 글자수 조절
        print!("  ");
        for (idx, &ch) in visual_blocks.iter().enumerate() {
            if idx > 0 && idx % cols == 0 {
                print!("\n  ");
            }
            match ch {
                'R' => print!("\x1b[32m[R]\x1b[0m"), // ANSI Green
                'T' => print!("\x1b[34m[T]\x1b[0m"), // ANSI Blue
                'B' => print!("\x1b[33m[B]\x1b[0m"), // ANSI Yellow
                'L' => print!("\x1b[90m[L]\x1b[0m"), // ANSI Bright Black (Grey)
                _ => {}
            }
        }
        println!("\n -------------------------------------------------------------");

        let total_blocks = literal_blocks + run_blocks + token_blocks + backref_blocks;
        println!(
            " * 통계: Total Blocks: {}, Literal: {} ({:.1}%), Run: {} ({:.1}%), Token: {} ({:.1}%), BackRef: {} ({:.1}%)",
            total_blocks,
            literal_blocks,
            (literal_blocks as f64 / total_blocks as f64) * 100.0,
            run_blocks,
            (run_blocks as f64 / total_blocks as f64) * 100.0,
            token_blocks,
            (token_blocks as f64 / total_blocks as f64) * 100.0,
            backref_blocks,
            (backref_blocks as f64 / total_blocks as f64) * 100.0
        );
        println!("===============================================================================");
    }

    if verified_status != "OK" {
        anyhow::bail!("경고: 체크섬 무결성 검증 실패!");
    }

    Ok(())
}
