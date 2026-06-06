use crate::format::{
    MzcHeader, MAGIC_MZC1, MAGIC_MZC2, MAGIC_MZC3, MAGIC_MZC4, MAGIC_MZC5, MAGIC_MZC6, MAGIC_MZC7,
    MAGIC_MZC8, MAGIC_MZC9,
};
use std::fs;
use std::path::Path;

/// **손상된 .mzc 또는 .mzar 아카이브 파일을 복구하여 지정된 디렉토리에 저장합니다.**
pub fn recover_archive(input_file: &Path, output_dir: &Path) -> Result<(), String> {
    let file_bytes =
        fs::read(input_file).map_err(|e| format!("입력 파일을 읽을 수 없습니다: {}", e))?;
    let recovered_entries = recover_bytes(&file_bytes)?;

    fs::create_dir_all(output_dir)
        .map_err(|e| format!("대상 디렉토리를 생성할 수 없습니다: {}", e))?;

    let mut file_count = 0;
    let mut dir_count = 0;

    for (rel_path, data) in recovered_entries {
        // 경로 구분선 정규화 및 상위 디렉토리 생성
        let clean_path = rel_path.replace('\\', "/");
        let out_path = output_dir.join(&clean_path);

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("부모 디렉토리 생성 실패: {}", e))?;
        }

        if clean_path.ends_with('/') || (data.is_empty() && clean_path.is_empty()) {
            fs::create_dir_all(&out_path).ok();
            dir_count += 1;
        } else {
            fs::write(&out_path, &data)
                .map_err(|e| format!("파일 쓰기 실패 ({:?}): {}", out_path, e))?;
            file_count += 1;
        }
    }

    println!(
        "성공적으로 복구 완료: 파일 {}개, 디렉토리 {}개",
        file_count, dir_count
    );
    Ok(())
}

/// **바이트 배열을 스캔하여 복구 가능한 파일 엔트리를 수집합니다.**
pub fn recover_bytes(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut recovered = Vec::new();
    let n = bytes.len();

    let mut i = 0;
    while i < n {
        let magic_slice = &bytes[i..std::cmp::min(i + 4, n)];

        // 1. MZAR 아카이브 컨테이너 감지 시 개별 엔트리 복구 시도
        if magic_slice == b"MZAR" {
            let mut pos = i + 8; // 매직(4) + 엔트리 개수(4) 건너뛰기
            while pos < n {
                if pos + 2 > n {
                    break;
                }
                let path_len = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]) as usize;
                pos += 2;

                if pos + path_len > n {
                    break;
                }

                if let Ok(path_str) = std::str::from_utf8(&bytes[pos..pos + path_len]) {
                    // 경로명 합당성 검증 (정상 문자열 검증)
                    let is_plausible = path_str.chars().all(|c| {
                        c.is_alphanumeric()
                            || c == '/'
                            || c == '_'
                            || c == '-'
                            || c == '.'
                            || c == ' '
                    });
                    if is_plausible && !path_str.is_empty() {
                        pos += path_len;
                        if pos + 9 > n {
                            break;
                        }
                        let entry_type = bytes[pos];
                        pos += 1;
                        let size =
                            u64::from_le_bytes(bytes[pos..pos + 8].try_into().unwrap()) as usize;
                        pos += 8;

                        if entry_type <= 2 && pos + size <= n {
                            let entry_data = &bytes[pos..pos + size];
                            pos += size;

                            if entry_type == 0 {
                                // 개별적으로 MZC 파일 압축이 적용된 상태인지 체크
                                if entry_data.len() >= 4
                                    && (&entry_data[0..4] == b"MZC1"
                                        || &entry_data[0..4] == b"MZC2"
                                        || &entry_data[0..4] == b"MZC3"
                                        || &entry_data[0..4] == b"MZC4"
                                        || &entry_data[0..4] == b"MZC5"
                                        || &entry_data[0..4] == b"MZC6"
                                        || &entry_data[0..4] == b"MZC7"
                                        || &entry_data[0..4] == b"MZC8"
                                        || &entry_data[0..4] == b"MZC9")
                                {
                                    if let Ok(decomp) = crate::decompress_bytes_v2(entry_data) {
                                        recovered.push((path_str.to_string(), decomp));
                                    } else {
                                        recovered.push((path_str.to_string(), entry_data.to_vec()));
                                    }
                                } else {
                                    recovered.push((path_str.to_string(), entry_data.to_vec()));
                                }
                            } else if entry_type == 1 {
                                recovered.push((format!("{}/", path_str), Vec::new()));
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            if pos > i {
                i = pos;
                continue;
            }
        }
        // 2. Standalone MZC 압축 바이트 감지 시 복제 해제 시도
        else if magic_slice == MAGIC_MZC1
            || magic_slice == MAGIC_MZC2
            || magic_slice == MAGIC_MZC3
            || magic_slice == MAGIC_MZC4
            || magic_slice == MAGIC_MZC5
            || magic_slice == MAGIC_MZC6
            || magic_slice == MAGIC_MZC7
            || magic_slice == MAGIC_MZC8
            || magic_slice == MAGIC_MZC9
        {
            if let Ok(header) = MzcHeader::from_bytes(&bytes[i..]) {
                let header_size = match header.version {
                    1 => 54,
                    9 => 64,
                    _ => 56,
                };
                let total_mzc_len = header_size + header.payload_size as usize;
                if i + total_mzc_len <= n {
                    let mzc_slice = &bytes[i..i + total_mzc_len];
                    if let Ok(decomp) = crate::decompress_bytes_v2(mzc_slice) {
                        // 복원된 결과 자체가 MZAR 아카이브 컨테이너일 수 있으므로 재귀 스캔 처리
                        if decomp.len() >= 4 && &decomp[0..4] == b"MZAR" {
                            if let Ok(sub_entries) = recover_bytes(&decomp) {
                                recovered.extend(sub_entries);
                            }
                        } else {
                            recovered.push((format!("recovered_file_{}.bin", i), decomp));
                        }
                        i += total_mzc_len;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    if recovered.is_empty() {
        return Err(
            "파일 내에서 감지 및 복구할 수 있는 유효한 데이터 엔트리를 찾지 못했습니다."
                .to_string(),
        );
    }

    // 경로 기준으로 중복 데이터 정리 (중복 참조 및 빈 디렉토리 제거 대비)
    let mut unique_entries = std::collections::HashMap::new();
    for (path, data) in recovered {
        unique_entries.entry(path).or_insert(data);
    }

    Ok(unique_entries.into_iter().collect())
}
