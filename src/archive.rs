use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

const MZAR_MAGIC: &[u8; 4] = b"MZAR";

/// **MZAR 아카이브 컨테이너 엔트리 메타데이터**
#[derive(Debug, Clone)]
pub struct MzarEntry {
    pub relative_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub data: Vec<u8>,
}

/// **지정한 디렉토리를 재귀적으로 순회하여 MZAR 컨테이너 바이트 배열로 직렬화합니다.**
pub fn archive_directory(src_dir: &Path) -> io::Result<Vec<u8>> {
    let mut paths = Vec::new();
    collect_paths(src_dir, src_dir, &mut paths)?;

    #[cfg(not(target_arch = "wasm32"))]
    let entries: Vec<MzarEntry> = paths
        .into_par_iter()
        .map(|(path, is_dir)| {
            let relative_path = path
                .strip_prefix(src_dir)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");

            if is_dir {
                Ok(MzarEntry {
                    relative_path,
                    is_dir: true,
                    size: 0,
                    data: Vec::new(),
                })
            } else {
                let data = fs::read(&path)?;
                let size = data.len() as u64;
                Ok(MzarEntry {
                    relative_path,
                    is_dir: false,
                    size,
                    data,
                })
            }
        })
        .collect::<Result<Vec<MzarEntry>, io::Error>>()?;

    #[cfg(target_arch = "wasm32")]
    let entries: Vec<MzarEntry> = paths
        .into_iter()
        .map(|(path, is_dir)| {
            let relative_path = path
                .strip_prefix(src_dir)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");

            if is_dir {
                Ok(MzarEntry {
                    relative_path,
                    is_dir: true,
                    size: 0,
                    data: Vec::new(),
                })
            } else {
                let data = fs::read(&path)?;
                let size = data.len() as u64;
                Ok(MzarEntry {
                    relative_path,
                    is_dir: false,
                    size,
                    data,
                })
            }
        })
        .collect::<Result<Vec<MzarEntry>, io::Error>>()?;

    let mut output = Vec::new();
    // 1. Magic bytes 쓰기
    output.write_all(MZAR_MAGIC)?;
    // 2. Entry 개수 쓰기 (u32)
    output.write_all(&(entries.len() as u32).to_le_bytes())?;

    // 3. 개별 엔트리 직렬화
    for entry in entries {
        let path_bytes = entry.relative_path.as_bytes();
        if path_bytes.len() > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("상대 경로가 너무 깁니다: {}", entry.relative_path),
            ));
        }
        // 경로 길이 (2B)
        output.write_all(&(path_bytes.len() as u16).to_le_bytes())?;
        // 경로 내용 (가변)
        output.write_all(path_bytes)?;
        // 디렉토리 플래그 (1B)
        output.write_all(&[if entry.is_dir { 1 } else { 0 }])?;
        // 파일 크기 (8B)
        output.write_all(&entry.size.to_le_bytes())?;
        // 파일 본문 (가변)
        if !entry.is_dir {
            output.write_all(&entry.data)?;
        }
    }

    Ok(output)
}

/// **지정한 여러 파일 및 디렉토리 경로들을 하나의 MZAR 컨테이너 바이트 배열로 직렬화합니다.**
pub fn archive_paths(paths: &[PathBuf]) -> io::Result<Vec<u8>> {
    let mut entries = Vec::new();

    for path in paths {
        let path = path.as_path();
        if !path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("경로를 찾을 수 없습니다: {:?}", path),
            ));
        }

        let file_name = path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "올바르지 않은 파일 이름"))?
            .to_string_lossy()
            .into_owned();

        if path.is_dir() {
            // 디렉토리 자체를 엔트리로 추가
            entries.push(MzarEntry {
                relative_path: file_name.replace('\\', "/"),
                is_dir: true,
                size: 0,
                data: Vec::new(),
            });

            // 내부 모든 하위 디렉토리 및 파일들을 수집
            let mut sub_paths = Vec::new();
            collect_paths(path, path, &mut sub_paths)?;

            for (sub_path, is_dir) in sub_paths {
                let rel = sub_path.strip_prefix(path).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
                })?;
                let relative_path = format!("{}/{}", file_name, rel.to_string_lossy())
                    .replace('\\', "/");

                if is_dir {
                    entries.push(MzarEntry {
                        relative_path,
                        is_dir: true,
                        size: 0,
                        data: Vec::new(),
                    });
                } else {
                    let data = fs::read(&sub_path)?;
                    let size = data.len() as u64;
                    entries.push(MzarEntry {
                        relative_path,
                        is_dir: false,
                        size,
                        data,
                    });
                }
            }
        } else {
            // 단일 파일 엔트리 추가
            let data = fs::read(path)?;
            let size = data.len() as u64;
            entries.push(MzarEntry {
                relative_path: file_name.replace('\\', "/"),
                is_dir: false,
                size,
                data,
            });
        }
    }

    let mut output = Vec::new();
    // 1. Magic bytes 쓰기
    output.write_all(MZAR_MAGIC)?;
    // 2. Entry 개수 쓰기 (u32)
    output.write_all(&(entries.len() as u32).to_le_bytes())?;

    // 3. 개별 엔트리 직렬화
    for entry in entries {
        let path_bytes = entry.relative_path.as_bytes();
        if path_bytes.len() > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("상대 경로가 너무 깁니다: {}", entry.relative_path),
            ));
        }
        // 경로 길이 (2B)
        output.write_all(&(path_bytes.len() as u16).to_le_bytes())?;
        // 경로 내용 (가변)
        output.write_all(path_bytes)?;
        // 디렉토리 플래그 (1B)
        output.write_all(&[if entry.is_dir { 1 } else { 0 }])?;
        // 파일 크기 (8B)
        output.write_all(&entry.size.to_le_bytes())?;
        // 파일 본문 (가변)
        if !entry.is_dir {
            output.write_all(&entry.data)?;
        }
    }

    Ok(output)
}

/// **MZAR 바이트 배열을 파싱하여 지정한 디렉토리에 풀어서 복원합니다.**
pub fn extract_archive(archive_bytes: &[u8], dest_dir: &Path) -> io::Result<()> {
    if archive_bytes.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "MZAR 데이터가 너무 짧습니다.",
        ));
    }

    // 1. Magic bytes 확인
    if &archive_bytes[0..4] != MZAR_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "유효한 MZAR 아카이브가 아닙니다.",
        ));
    }

    // 2. Entry 개수 읽기
    let mut entry_count_bytes = [0u8; 4];
    entry_count_bytes.copy_from_slice(&archive_bytes[4..8]);
    let entry_count = u32::from_le_bytes(entry_count_bytes);

    let mut cursor = 8;
    let data_len = archive_bytes.len();

    // 대상 디렉토리를 물리적으로 생성
    fs::create_dir_all(dest_dir)?;
    let canonical_dest = dest_dir.canonicalize()?;

    for _ in 0..entry_count {
        if cursor + 2 > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "엔트리 헤더 읽기 실패: 경량 헤더",
            ));
        }

        // 경로 길이 (2B)
        let mut path_len_bytes = [0u8; 2];
        path_len_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 2]);
        let path_len = u16::from_le_bytes(path_len_bytes) as usize;
        cursor += 2;

        if cursor + path_len > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "상대 경로 바이트 읽기 실패",
            ));
        }

        // 경로명 디코딩
        let path_str = std::str::from_utf8(&archive_bytes[cursor..cursor + path_len])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        cursor += path_len;

        if cursor + 9 > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "메타데이터 플래그 읽기 실패",
            ));
        }

        // 디렉토리 여부 (1B)
        let is_dir = archive_bytes[cursor] == 1;
        cursor += 1;

        // 파일 크기 (8B)
        let mut file_size_bytes = [0u8; 8];
        file_size_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 8]);
        let file_size = u64::from_le_bytes(file_size_bytes) as usize;
        cursor += 8;

        // 타겟 경로 매핑 및 Zip-Slip 보안 취약성 방어
        let target_path = dest_dir.join(path_str);

        // zip-slip 방지: 상위 폴더 경로로 탈출하는 공격 방어
        if is_dir {
            fs::create_dir_all(&target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        // 존재 여부에 따른 부모 디렉토리 canonicalize 검증 우회
        let canonical_target =
            if target_path.exists() {
                target_path.canonicalize()?
            } else {
                let parent_canonical = target_path
                    .parent()
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "부모 디렉토리가 없습니다.")
                    })?
                    .canonicalize()?;
                parent_canonical.join(target_path.file_name().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "파일명이 없습니다.")
                })?)
            };

        if !canonical_target.starts_with(&canonical_dest) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("Zip-Slip 위협 차단: 대상 외 경로 탐지: {:?}", target_path),
            ));
        }

        if is_dir {
            // 디렉토리 생성 완료
        } else {
            if cursor + file_size > data_len {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "파일 데이터가 잘렸습니다.",
                ));
            }
            let file_data = &archive_bytes[cursor..cursor + file_size];
            cursor += file_size;

            fs::write(&target_path, file_data)?;
        }
    }

    Ok(())
}

/// **디렉토리 재귀 탐색 헬퍼 함수 (경로 수집)**
fn collect_paths(
    base_dir: &Path,
    current_dir: &Path,
    paths: &mut Vec<(PathBuf, bool)>,
) -> io::Result<()> {
    for entry_res in fs::read_dir(current_dir)? {
        let entry = entry_res?;
        let path = entry.path();

        // base_dir 기준 상대 경로 계산
        let relative_path = path
            .strip_prefix(base_dir)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            paths.push((path.clone(), true));
            // 재귀 호출
            collect_paths(base_dir, &path, paths)?;
        } else if file_type.is_file() {
            paths.push((path, false));
        }
    }
    Ok(())
}

/// **주어진 바이트 어레이가 MZAR 컨테이너 헤더로 시작하는지 검사합니다.**
pub fn is_mzar_archive(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[0..4] == MZAR_MAGIC
}

/// **MZAR 아카이브 엔트리의 경량 메타데이터 구조체입니다.**
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MzarEntryMetadata {
    pub relative_path: String,
    pub is_dir: bool,
    pub size: u64,
}

/// **MZAR 바이트 배열을 파싱하여 엔트리 메타데이터의 목록을 가져옵니다 (추출하지 않음).**
pub fn parse_mzar_metadata(archive_bytes: &[u8]) -> io::Result<Vec<MzarEntryMetadata>> {
    if archive_bytes.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "MZAR 데이터가 너무 짧습니다.",
        ));
    }

    // 1. Magic bytes 확인
    if &archive_bytes[0..4] != MZAR_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "유효한 MZAR 아카이브가 아닙니다.",
        ));
    }

    // 2. Entry 개수 읽기
    let mut entry_count_bytes = [0u8; 4];
    entry_count_bytes.copy_from_slice(&archive_bytes[4..8]);
    let entry_count = u32::from_le_bytes(entry_count_bytes);

    let mut cursor = 8;
    let data_len = archive_bytes.len();
    let mut entries = Vec::new();

    for _ in 0..entry_count {
        if cursor + 2 > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "엔트리 헤더 읽기 실패: 경량 헤더",
            ));
        }

        // 경로 길이 (2B)
        let mut path_len_bytes = [0u8; 2];
        path_len_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 2]);
        let path_len = u16::from_le_bytes(path_len_bytes) as usize;
        cursor += 2;

        if cursor + path_len > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "상대 경로 바이트 읽기 실패",
            ));
        }

        // 경로명 디코딩
        let path_str = std::str::from_utf8(&archive_bytes[cursor..cursor + path_len])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
            .to_string();
        cursor += path_len;

        if cursor + 9 > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "메타데이터 플래그 읽기 실패",
            ));
        }

        // 디렉토리 여부 (1B)
        let is_dir = archive_bytes[cursor] == 1;
        cursor += 1;

        // 파일 크기 (8B)
        let mut file_size_bytes = [0u8; 8];
        file_size_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 8]);
        let file_size = u64::from_le_bytes(file_size_bytes);
        cursor += 8;

        entries.push(MzarEntryMetadata {
            relative_path: path_str,
            is_dir,
            size: file_size,
        });

        if !is_dir {
            if cursor + (file_size as usize) > data_len {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "파일 데이터 범위를 초과했습니다.",
                ));
            }
            cursor += file_size as usize;
        }
    }

    Ok(entries)
}

