use crate::checksum::{bytes_to_hex, calculate_sha256};
use crate::decompress_bytes_v2;
use crate::format::{
    MzcHeader, ALGORITHM_DICT, ALGORITHM_HYBRID, ALGORITHM_LZ77, ALGORITHM_RLE,
    ALGORITHM_DEFLATE, ALGORITHM_ZSTD, HEADER_SIZE_MZC1,
    HEADER_SIZE_MZC2, HEADER_SIZE_MZC9, VERSION_MZC1, VERSION_MZC2, VERSION_MZC7, VERSION_MZC8, VERSION_MZC9,
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

    // MZAR 컨테이너(비솔리드 아카이브)인 경우 우선 처리
    if crate::archive::is_mzar_archive(&file_bytes) {
        return inspect_mzar_non_solid(path, &file_bytes);
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
    let mut is_encrypted_file = false;
    let decompressed_res = decompress_bytes_v2(&file_bytes);
    
    let mut decompressed_opt = None;
    let mut computed_hash_str = String::new();
    let verified_status = match decompressed_res {
        Ok(decompressed) => {
            let computed_hash = calculate_sha256(&decompressed);
            computed_hash_str = bytes_to_hex(&computed_hash);
            let status = if computed_hash == header.original_sha256 {
                "OK".to_string()
            } else {
                "FAILED (SHA-256 Mismatch)".to_string()
            };
            decompressed_opt = Some(decompressed);
            status
        }
        Err(crate::error::MzcError::PasswordRequired) => {
            is_encrypted_file = true;
            "Encrypted (Password required)".to_string()
        }
        Err(mzc_err) => {
            return Err(anyhow::anyhow!(mzc_err).context("이진 데이터 압축 해제 및 무결성(Integrity) 검증에 실패했습니다."));
        }
    };

    let original_hash_str = bytes_to_hex(&header.original_sha256);

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

    let mode_str = if header.version == VERSION_MZC7 || header.version == VERSION_MZC9 {
        if header.algorithm_type == ALGORITHM_DEFLATE {
            "Deflate".to_string()
        } else if header.algorithm_type == ALGORITHM_ZSTD {
            "Zstd".to_string()
        } else {
            let core_bits = header.algorithm_type & 0x03;
            let entropy_bits = (header.algorithm_type >> 2) & 0x07;
            let filter_bits = (header.algorithm_type >> 5) & 0x07;

            let core_name = match core_bits {
                0 => "RLE",
                1 => "Dictionary Only",
                2 => "Hybrid",
                3 => "LZ77",
                _ => "Unknown",
            };
            let entropy_name = match entropy_bits {
                0 => "None",
                1 => "Huffman",
                2 => "Dynamic",
                3 => "ANS",
                4 => "CM (Context Mixing)",
                _ => "Unknown",
            };
            let filter_name = match filter_bits {
                0 => "None",
                1 => "Delta",
                2 => "BCJ",
                3 => "PNG",
                4 => "LPC",
                5 => "Delta+BCJ",
                6 => "BWT",
                _ => "Unknown",
            };
            format!("{} + {} (Filter: {})", core_name, entropy_name, filter_name)
        }
    } else {
        let core_alg = header.algorithm_type & 0x0F;
        let core_name = match core_alg {
            ALGORITHM_RLE => "RLE Only (Run-Length Encoding)",
            ALGORITHM_DICT => "Dictionary Only (Entropy Enabled)",
            ALGORITHM_HYBRID => "Hybrid Mode (RLE + Dictionary + Static Huffman)",
            ALGORITHM_LZ77 => "LZ77 Hybrid Mode (Runs + Dictionary + BackRefs + Huffman)",
            _ => "Unknown Mode",
        };
        core_name.to_string()
    };

    let is_solid_archive = decompressed_opt.as_ref().map(|d| crate::archive::is_mzar_archive(d)).unwrap_or(false);
    let archive_type_str = if is_encrypted_file {
        "Encrypted File/Archive"
    } else if is_solid_archive {
        "Solid Archive (Compressed MZAR Container)"
    } else {
        "Single Compressed File"
    };

    // 5. 결과 기본 정보 출력
    println!("===============================================================================");
    println!(" File: {:?}", path.file_name().unwrap_or(path.as_os_str()));
    println!(" Format: {}", format_str);
    println!(" Archive Type: {}", archive_type_str);
    println!(" Algorithm: {}", mode_str);
    println!(" Original size: {} bytes", original_size);
    println!(" Compressed size: {} bytes", total_compressed_size);
    println!(" Ratio: {:.2}%", ratio);
    println!(" SHA-256: {}", original_hash_str);
    if verified_status != "OK" && !is_encrypted_file {
        println!(" Computed SHA-256: {}", computed_hash_str);
    }
    println!(" Verified: {}", verified_status);
    println!("===============================================================================");

    // 솔리드 아카이브인 경우 아카이브 엔트리 테이블 출력
    if is_solid_archive {
        if let Some(ref decompressed) = decompressed_opt {
            if let Ok(entries) = parse_inspect_entries(decompressed) {
                let total_entries = entries.len();
                let dir_count = entries.iter().filter(|e| e.entry_type == 1).count();
                let file_count = entries.iter().filter(|e| e.entry_type == 0).count();
                let dup_count = entries.iter().filter(|e| e.entry_type == 2).count();

                println!("[ Archive Entries (Solid Mode) ]");
                println!(" {:<4} | {:<10} | {:>15} | Path", "No.", "Type", "Size");
                println!("-------------------------------------------------------------------------------");
                for (idx, entry) in entries.iter().enumerate() {
                    let plain_type = match entry.entry_type {
                        0 => "File",
                        1 => "Directory",
                        2 => "Duplicate",
                        _ => "Unknown",
                    };
                    let type_str = match entry.entry_type {
                        0 => format!("\x1b[32m{:<10}\x1b[0m", plain_type),
                        1 => format!("\x1b[34m{:<10}\x1b[0m", plain_type),
                        2 => format!("\x1b[36m{:<10}\x1b[0m", plain_type),
                        _ => format!("{:<10}", plain_type),
                    };
                    let size_str = if entry.entry_type == 1 {
                        "-".to_string()
                    } else if entry.entry_type == 2 {
                        "-".to_string()
                    } else {
                        format!("{} B", entry.original_size)
                    };
                    let path_str = if entry.entry_type == 2 {
                        format!("{} \x1b[90m(Duplicate reference)\x1b[0m", entry.path)
                    } else {
                        entry.path.clone()
                    };
                    println!(" {:<4} | {} | {:>15} | {}", idx + 1, type_str, size_str, path_str);
                }
                println!("-------------------------------------------------------------------------------");
                println!(" Total: {} entries ({} files, {} directories, {} duplicates)", 
                    total_entries, file_count, dir_count, dup_count);
                println!("===============================================================================");
            }
        }
    }

    // 6. 페이로드 블록 분석 및 ANSI 컬러 비주얼 맵 렌더링
    // 페이로드 영역의 가변 청크 내부 블록(Literal: L, Run: R, Token: T, BackRef: B)들의 분포를 스캔해 수집합니다.
    let mut literal_blocks = 0;
    let mut run_blocks = 0;
    let mut token_blocks = 0;
    let mut backref_blocks = 0;
    let mut visual_blocks = Vec::new();

    if header.version >= VERSION_MZC2 && original_size > 0 
        && header.algorithm_type != ALGORITHM_DEFLATE 
        && header.algorithm_type != ALGORITHM_ZSTD 
    {
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

struct InspectEntry {
    path: String,
    entry_type: u8,
    original_size: u64,
    compressed_size: Option<u64>,
    method: String,
}

fn parse_inspect_entries(archive_bytes: &[u8]) -> Result<Vec<InspectEntry>> {
    if archive_bytes.len() < 8 {
        anyhow::bail!("MZAR 데이터가 너무 짧습니다.");
    }
    if &archive_bytes[0..4] != b"MZAR" {
        anyhow::bail!("유효한 MZAR 아카이브가 아닙니다.");
    }
    let mut entry_count_bytes = [0u8; 4];
    entry_count_bytes.copy_from_slice(&archive_bytes[4..8]);
    let entry_count = u32::from_le_bytes(entry_count_bytes);

    let mut cursor = 8;
    let data_len = archive_bytes.len();
    let mut entries = Vec::new();

    for _ in 0..entry_count {
        if cursor + 2 > data_len {
            anyhow::bail!("엔트리 헤더 읽기 실패: 경량 헤더");
        }
        let mut path_len_bytes = [0u8; 2];
        path_len_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 2]);
        let path_len = u16::from_le_bytes(path_len_bytes) as usize;
        cursor += 2;

        if cursor + path_len > data_len {
            anyhow::bail!("상대 경로 바이트 읽기 실패");
        }
        let path_str = std::str::from_utf8(&archive_bytes[cursor..cursor + path_len])
            .context("상대 경로 UTF-8 디코딩 실패")?
            .to_string();
        cursor += path_len;

        if cursor + 9 > data_len {
            anyhow::bail!("메타데이터 플래그 읽기 실패");
        }
        let entry_type = archive_bytes[cursor];
        cursor += 1;

        let mut file_size_bytes = [0u8; 8];
        file_size_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 8]);
        let file_size = u64::from_le_bytes(file_size_bytes);
        cursor += 8;

        let mut original_size = file_size;
        let mut compressed_size = None;
        let mut method = "Uncompressed".to_string();

        if entry_type == 0 {
            let size_usize = file_size as usize;
            if cursor + size_usize > data_len {
                anyhow::bail!("파일 데이터 범위를 초과했습니다.");
            }
            let entry_data_slice = &archive_bytes[cursor..cursor + size_usize];
            cursor += size_usize;

            if entry_data_slice.len() >= 4 && &entry_data_slice[0..3] == b"MZC" {
                if let Ok(mzc_h) = MzcHeader::from_bytes(entry_data_slice) {
                    original_size = mzc_h.original_size;
                    compressed_size = Some(file_size);
                    
                    let format_str = if mzc_h.version == VERSION_MZC9 {
                        "MZC9".to_string()
                    } else if mzc_h.version >= VERSION_MZC2 {
                        format!("MZC{}", mzc_h.version)
                    } else {
                        "MZC1".to_string()
                    };

                    let mode_str = if mzc_h.algorithm_type == ALGORITHM_DEFLATE {
                        "Deflate".to_string()
                    } else if mzc_h.algorithm_type == ALGORITHM_ZSTD {
                        "Zstd".to_string()
                    } else {
                        let parsed = match mzc_h.algorithm_type & 0x0F {
                            ALGORITHM_RLE => "Rle",
                            ALGORITHM_DICT => "Dict",
                            ALGORITHM_HYBRID => "Hybrid",
                            ALGORITHM_LZ77 => "LZ77",
                            _ => "Unknown",
                        };
                        parsed.to_string()
                    };
                    
                    let is_encrypted = mzc_h.version == VERSION_MZC8 || (mzc_h.version == VERSION_MZC9 && (mzc_h.checksum_type & 0x80) != 0);
                    let enc_suffix = if is_encrypted { " [Encrypted]" } else { "" };
                    method = format!("{}{} ({})", format_str, enc_suffix, mode_str);
                }
            }
        } else if entry_type == 2 {
            let size_usize = file_size as usize;
            if cursor + size_usize > data_len {
                anyhow::bail!("중복 참조 경로가 잘렸습니다.");
            }
            let ref_bytes = &archive_bytes[cursor..cursor + size_usize];
            cursor += size_usize;
            let ref_str = std::str::from_utf8(ref_bytes)
                .context("중복 참조 경로 UTF-8 디코딩 실패")?;
            method = format!("Duplicate (-> {})", ref_str);
            original_size = 0;
        } else if entry_type == 1 {
            method = "-".to_string();
            original_size = 0;
        }

        entries.push(InspectEntry {
            path: path_str,
            entry_type,
            original_size,
            compressed_size,
            method,
        });
    }

    Ok(entries)
}

fn inspect_mzar_non_solid(path: &Path, file_bytes: &[u8]) -> Result<()> {
    let entries = parse_inspect_entries(file_bytes)
        .context("MZAR 아카이브 엔트리 파싱 실패")?;

    let total_entries = entries.len();
    let dir_count = entries.iter().filter(|e| e.entry_type == 1).count();
    let file_count = entries.iter().filter(|e| e.entry_type == 0).count();
    let dup_count = entries.iter().filter(|e| e.entry_type == 2).count();

    let total_orig_size: u64 = entries.iter().map(|e| e.original_size).sum();
    let total_disk_size = file_bytes.len() as u64;
    let ratio = if total_orig_size > 0 {
        (total_disk_size as f64 / total_orig_size as f64) * 100.0
    } else {
        100.0
    };

    println!("===============================================================================");
    println!(" File: {:?}", path.file_name().unwrap_or(path.as_os_str()));
    println!(" Format: MZAR Archive (Minimal Zip Archive Container)");
    println!(" Archive Type: Non-Solid Archive (Individual entry compression)");
    println!(" Entries Count: {}", total_entries);
    println!(" Total Original size: {} bytes", total_orig_size);
    println!(" Total Compressed size (on disk): {} bytes", total_disk_size);
    println!(" Overall Ratio: {:.2}%", ratio);
    println!("===============================================================================");
    println!("[ Archive Entries (Non-Solid Mode) ]");
    println!(" {:<4} | {:<10} | {:>15} | {:>15} | {:<25} | Path", "No.", "Type", "Orig Size", "Comp Size", "Method");
    println!("-------------------------------------------------------------------------------");
    for (idx, entry) in entries.iter().enumerate() {
        let plain_type = match entry.entry_type {
            0 => "File",
            1 => "Directory",
            2 => "Duplicate",
            _ => "Unknown",
        };
        let type_str = match entry.entry_type {
            0 => format!("\x1b[32m{:<10}\x1b[0m", plain_type),
            1 => format!("\x1b[34m{:<10}\x1b[0m", plain_type),
            2 => format!("\x1b[36m{:<10}\x1b[0m", plain_type),
            _ => format!("{:<10}", plain_type),
        };
        let orig_size_str = if entry.entry_type == 1 {
            "-".to_string()
        } else if entry.entry_type == 2 {
            "-".to_string()
        } else {
            format!("{} B", entry.original_size)
        };
        let comp_size_str = if entry.entry_type == 0 {
            if let Some(cs) = entry.compressed_size {
                format!("{} B", cs)
            } else {
                "Stored".to_string()
            }
        } else {
            "-".to_string()
        };
        let mut display_method = entry.method.clone();
        if display_method.len() > 25 {
            display_method = format!("{}..", &display_method[..23]);
        }
        let method_str = if entry.entry_type == 0 {
            if entry.compressed_size.is_some() {
                format!("\x1b[32m{:<25}\x1b[0m", display_method)
            } else {
                format!("\x1b[90m{:<25}\x1b[0m", display_method)
            }
        } else if entry.entry_type == 2 {
            format!("\x1b[36m{:<25}\x1b[0m", display_method)
        } else {
            format!("\x1b[90m{:<25}\x1b[0m", "-")
        };
        println!(" {:<4} | {} | {:>15} | {:>15} | {} | {}", 
            idx + 1, type_str, orig_size_str, comp_size_str, method_str, entry.path);
    }
    println!("-------------------------------------------------------------------------------");
    println!(" Total: {} entries ({} files, {} directories, {} duplicates)", 
        total_entries, file_count, dir_count, dup_count);
    println!("===============================================================================");

    Ok(())
}
