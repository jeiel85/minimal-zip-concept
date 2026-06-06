use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::cli::{CompressionMode, EntropyMode};

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

const MZAR_MAGIC: &[u8; 4] = b"MZAR";

/// **MZAR 아카이브 압축을 위한 개별 압축 파라미터 구조체**
#[derive(Debug, Clone)]
pub struct CompressionParams<'a> {
    pub mode: CompressionMode,
    pub entropy: EntropyMode,
    pub level: u8,
    pub delta: bool,
    pub bcj: bool,
    pub png: bool,
    pub lpc: bool,
    pub bwt: bool,
    pub dict_data: Option<&'a [u8]>,
    pub password: Option<&'a str>,
    pub chunk_size: Option<u32>,
    pub checksum_type: u8,
}

/// **MZAR 아카이브 컨테이너 엔트리 메타데이터**
#[derive(Debug, Clone)]
pub struct MzarEntry {
    pub relative_path: String,
    pub entry_type: u8, // 0 = File, 1 = Directory, 2 = Duplicate Reference
    pub size: u64,
    pub data: Vec<u8>,
}

/// **지정한 디렉토리를 재귀적으로 순회하여 MZAR 컨테이너 바이트 배열로 직렬화합니다.**
pub fn archive_directory(src_dir: &Path) -> io::Result<Vec<u8>> {
    archive_directory_custom(src_dir, None)
}

/// **지정한 디렉토리를 재귀적으로 순회하여 개별/솔리드 압축 옵션을 받아 MZAR 컨테이너 바이트 배열로 직렬화합니다.**
pub fn archive_directory_custom(
    src_dir: &Path,
    compress_params: Option<&CompressionParams>,
) -> io::Result<Vec<u8>> {
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
                    entry_type: 1,
                    size: 0,
                    data: Vec::new(),
                })
            } else {
                let data = fs::read(&path)?;
                let size = data.len() as u64;
                Ok(MzarEntry {
                    relative_path,
                    entry_type: 0,
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
                    entry_type: 1,
                    size: 0,
                    data: Vec::new(),
                })
            } else {
                let data = fs::read(&path)?;
                let size = data.len() as u64;
                Ok(MzarEntry {
                    relative_path,
                    entry_type: 0,
                    size,
                    data,
                })
            }
        })
        .collect::<Result<Vec<MzarEntry>, io::Error>>()?;

    serialize_entries_custom(entries, compress_params)
}

/// **지정한 여러 파일 및 디렉토리 경로들을 하나의 MZAR 컨테이너 바이트 배열로 직렬화합니다.**
pub fn archive_paths(paths: &[PathBuf]) -> io::Result<Vec<u8>> {
    archive_paths_custom(paths, None)
}

/// **지정한 여러 파일 및 디렉토리 경로들을 개별/솔리드 압축 옵션을 받아 하나의 MZAR 컨테이너 바이트 배열로 직렬화합니다.**
pub fn archive_paths_custom(
    paths: &[PathBuf],
    compress_params: Option<&CompressionParams>,
) -> io::Result<Vec<u8>> {
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
                entry_type: 1,
                size: 0,
                data: Vec::new(),
            });

            // 내부 모든 하위 디렉토리 및 파일들을 수집
            let mut sub_paths = Vec::new();
            collect_paths(path, path, &mut sub_paths)?;

            for (sub_path, is_dir) in sub_paths {
                let rel = sub_path
                    .strip_prefix(path)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
                let relative_path =
                    format!("{}/{}", file_name, rel.to_string_lossy()).replace('\\', "/");

                if is_dir {
                    entries.push(MzarEntry {
                        relative_path,
                        entry_type: 1,
                        size: 0,
                        data: Vec::new(),
                    });
                } else {
                    let data = fs::read(&sub_path)?;
                    let size = data.len() as u64;
                    entries.push(MzarEntry {
                        relative_path,
                        entry_type: 0,
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
                entry_type: 0,
                size,
                data,
            });
        }
    }

    serialize_entries_custom(entries, compress_params)
}

/// **엔트리 목록을 받아서 중복 데이터를 탐색해 Deduplication을 수행한 후 직렬화합니다.**
pub fn serialize_entries(entries: Vec<MzarEntry>) -> io::Result<Vec<u8>> {
    serialize_entries_custom(entries, None)
}

/// **엔트리 목록을 받아서 중복 제거 및 (비솔리드인 경우) 개별 병렬 압축을 수행한 뒤 직렬화합니다.**
pub fn serialize_entries_custom(
    mut entries: Vec<MzarEntry>,
    compress_params: Option<&CompressionParams>,
) -> io::Result<Vec<u8>> {
    // 중복 제거 매핑: 파일 본문 해시 대신 데이터를 그대로 Map의 키로 매핑
    let mut seen_files: std::collections::HashMap<Vec<u8>, String> =
        std::collections::HashMap::new();
    for entry in entries.iter_mut() {
        if entry.entry_type == 0 {
            if let Some(first_path) = seen_files.get(&entry.data) {
                entry.entry_type = 2; // 중복 참조 타입
                entry.data = first_path.as_bytes().to_vec();
                entry.size = entry.data.len() as u64;
            } else {
                seen_files.insert(entry.data.clone(), entry.relative_path.clone());
            }
        }
    }

    // 개별 파일 압축 (비솔리드 모드 활성화 시)
    if let Some(params) = compress_params {
        #[cfg(not(target_arch = "wasm32"))]
        {
            entries
                .par_iter_mut()
                .filter(|entry| entry.entry_type == 0)
                .for_each(|entry| {
                    let compressed = crate::compress_bytes_v2_with_progress_dict_password(
                        &entry.data,
                        params.mode,
                        params.entropy,
                        params.level,
                        params.delta,
                        params.bcj,
                        params.png,
                        params.lpc,
                        params.bwt,
                        params.dict_data,
                        params.password,
                        params.chunk_size,
                        params.checksum_type,
                        |_, _, _, _| {},
                    );
                    entry.data = compressed;
                    entry.size = entry.data.len() as u64;
                });
        }
        #[cfg(target_arch = "wasm32")]
        {
            for entry in entries.iter_mut() {
                if entry.entry_type == 0 {
                    let compressed = crate::compress_bytes_v2_with_progress_dict_password(
                        &entry.data,
                        params.mode,
                        params.entropy,
                        params.level,
                        params.delta,
                        params.bcj,
                        params.png,
                        params.lpc,
                        params.bwt,
                        params.dict_data,
                        params.password,
                        params.chunk_size,
                        params.checksum_type,
                        |_, _, _, _| {},
                    );
                    entry.data = compressed;
                    entry.size = entry.data.len() as u64;
                }
            }
        }
    }

    let mut output = Vec::new();
    output.write_all(MZAR_MAGIC)?;
    output.write_all(&(entries.len() as u32).to_le_bytes())?;

    for entry in entries {
        let path_bytes = entry.relative_path.as_bytes();
        if path_bytes.len() > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("상대 경로가 너무 깁니다: {}", entry.relative_path),
            ));
        }
        output.write_all(&(path_bytes.len() as u16).to_le_bytes())?;
        output.write_all(path_bytes)?;
        output.write_all(&[entry.entry_type])?;
        output.write_all(&entry.size.to_le_bytes())?;
        if entry.entry_type == 0 || entry.entry_type == 2 {
            output.write_all(&entry.data)?;
        }
    }

    Ok(output)
}

/// **MZAR 바이트 배열을 파싱하여 지정한 디렉토리에 풀어서 복원합니다. 개별 압축 파일의 해제 및 복호화도 지원합니다.**
pub fn extract_archive(
    archive_bytes: &[u8],
    dest_dir: &Path,
    password: Option<&str>,
    dict_data: Option<&[u8]>,
) -> io::Result<()> {
    if archive_bytes.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "MZAR 데이터가 너무 짧습니다.",
        ));
    }

    if &archive_bytes[0..4] != MZAR_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "유효한 MZAR 아카이브가 아닙니다.",
        ));
    }

    let mut entry_count_bytes = [0u8; 4];
    entry_count_bytes.copy_from_slice(&archive_bytes[4..8]);
    let entry_count = u32::from_le_bytes(entry_count_bytes);

    let mut cursor = 8;
    let data_len = archive_bytes.len();

    fs::create_dir_all(dest_dir)?;
    let canonical_dest = dest_dir.canonicalize().or_else(|_| {
        fs::create_dir_all(dest_dir)?;
        dest_dir.canonicalize()
    })?;

    let mut dirs_to_create = Vec::new();
    let mut files_to_write = Vec::new();
    let mut duplicates = Vec::new();

    for _ in 0..entry_count {
        if cursor + 2 > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "엔트리 헤더 읽기 실패: 경량 헤더",
            ));
        }

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

        let path_str = std::str::from_utf8(&archive_bytes[cursor..cursor + path_len])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        cursor += path_len;

        if cursor + 9 > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "메타데이터 플래그 읽기 실패",
            ));
        }

        let entry_type = archive_bytes[cursor];
        cursor += 1;

        let mut file_size_bytes = [0u8; 8];
        file_size_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 8]);
        let file_size = u64::from_le_bytes(file_size_bytes) as usize;
        cursor += 8;

        let target_path = dest_dir.join(path_str);

        let canonical_target =
            if target_path.exists() {
                target_path.canonicalize()?
            } else {
                let parent_canonical = target_path
                    .parent()
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "부모 디렉토리가 없습니다.")
                    })?
                    .canonicalize()
                    .or_else(|_| {
                        if let Some(p) = target_path.parent() {
                            fs::create_dir_all(p)?;
                            p.canonicalize()
                        } else {
                            Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "부모 디렉토리 생성 실패",
                            ))
                        }
                    })?;
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

        if entry_type == 1 {
            dirs_to_create.push(target_path);
        } else if entry_type == 0 {
            if cursor + file_size > data_len {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "파일 데이터가 잘렸습니다.",
                ));
            }
            let mut file_data = archive_bytes[cursor..cursor + file_size].to_vec();
            cursor += file_size;

            if file_data.len() >= 4 && &file_data[0..3] == b"MZC" {
                file_data =
                    crate::decompress_bytes_v2_dict_password(&file_data, dict_data, password)
                        .map_err(|e| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("개별 파일 해제 실패: {:?}", e),
                            )
                        })?;
            }

            files_to_write.push((target_path, file_data));
        } else if entry_type == 2 {
            if cursor + file_size > data_len {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "중복 참조 경로가 잘렸습니다.",
                ));
            }
            let ref_bytes = &archive_bytes[cursor..cursor + file_size];
            cursor += file_size;
            let ref_str = std::str::from_utf8(ref_bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
                .to_string();
            duplicates.push((target_path, ref_str));
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("알 수 없는 엔트리 타입: {}", entry_type),
            ));
        }
    }

    // 1. 디렉토리 생성 병렬 처리
    #[cfg(not(target_arch = "wasm32"))]
    {
        dirs_to_create
            .par_iter()
            .try_for_each(|dir| fs::create_dir_all(dir))?;
    }
    #[cfg(target_arch = "wasm32")]
    {
        for dir in &dirs_to_create {
            fs::create_dir_all(dir)?;
        }
    }

    // 2. 파일 쓰기 병렬 처리
    #[cfg(not(target_arch = "wasm32"))]
    {
        files_to_write.par_iter().try_for_each(|(path, data)| {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, data)
        })?;
    }
    #[cfg(target_arch = "wasm32")]
    {
        for (path, data) in &files_to_write {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, data)?;
        }
    }

    // 3. 중복 파일 복사 순차 처리
    for (target_path, ref_str) in duplicates {
        let source_path = dest_dir.join(&ref_str);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&source_path, &target_path)?;
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

        let relative_path = path
            .strip_prefix(base_dir)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            paths.push((path.clone(), true));
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
    pub entry_type: u8, // 0 = File, 1 = Directory, 2 = Duplicate Reference
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

    if &archive_bytes[0..4] != MZAR_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "유효한 MZAR 아카이브가 아닙니다.",
        ));
    }

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

        let entry_type = archive_bytes[cursor];
        let is_dir = entry_type == 1;
        cursor += 1;

        let mut file_size_bytes = [0u8; 8];
        file_size_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 8]);
        let file_size = u64::from_le_bytes(file_size_bytes);
        cursor += 8;

        let mut resolved_size = file_size;
        if entry_type == 0 || entry_type == 2 {
            if cursor + (file_size as usize) > data_len {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "파일 데이터 범위를 초과했습니다.",
                ));
            }
            if entry_type == 0 && file_size >= 4 {
                let entry_data_slice = &archive_bytes[cursor..cursor + (file_size as usize)];
                if entry_data_slice.len() >= 4 && &entry_data_slice[0..3] == b"MZC" {
                    if let Ok(mzc_h) = crate::format::MzcHeader::from_bytes(entry_data_slice) {
                        resolved_size = mzc_h.original_size;
                    }
                }
            }
            cursor += file_size as usize;
        }

        entries.push(MzarEntryMetadata {
            relative_path: path_str,
            entry_type,
            is_dir,
            size: resolved_size,
        });
    }

    Ok(entries)
}

/// **MZAR 바이트 배열에서 단일 파일을 타겟팅해 인메모리로 추출하여 반환합니다. 중복 참조도 완벽히 추적 및 역참조하며, 개별 압축도 해제합니다.**
pub fn extract_single_file_from_mzar(
    archive_bytes: &[u8],
    target_rel_path: &str,
    password: Option<&str>,
    dict_data: Option<&[u8]>,
) -> io::Result<Vec<u8>> {
    if archive_bytes.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "MZAR 데이터가 너무 짧습니다.",
        ));
    }
    if &archive_bytes[0..4] != MZAR_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "유효한 MZAR 아카이브가 아닙니다.",
        ));
    }

    let mut entry_count_bytes = [0u8; 4];
    entry_count_bytes.copy_from_slice(&archive_bytes[4..8]);
    let entry_count = u32::from_le_bytes(entry_count_bytes);

    let mut cursor = 8;
    let data_len = archive_bytes.len();

    let mut entries_map = std::collections::HashMap::new();

    for _ in 0..entry_count {
        if cursor + 2 > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "엔트리 헤더 읽기 실패: 경량 헤더",
            ));
        }
        let mut path_len_bytes = [0u8; 2];
        path_len_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 2]);
        let path_len = u16::from_le_bytes(path_len_bytes) as usize;
        cursor += 2;

        if cursor + path_len > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "상대 경로 읽기 실패",
            ));
        }
        let path_str = std::str::from_utf8(&archive_bytes[cursor..cursor + path_len])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
            .to_string();
        cursor += path_len;

        if cursor + 9 > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "플래그 읽기 실패",
            ));
        }
        let entry_type = archive_bytes[cursor];
        cursor += 1;

        let mut file_size_bytes = [0u8; 8];
        file_size_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 8]);
        let file_size = u64::from_le_bytes(file_size_bytes) as usize;
        cursor += 8;

        let data_offset = cursor;
        if entry_type == 0 || entry_type == 2 {
            if cursor + file_size > data_len {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "파일 데이터가 잘렸습니다.",
                ));
            }
            cursor += file_size;
        }

        entries_map.insert(path_str, (entry_type, file_size, data_offset));
    }

    let normalized_target = target_rel_path.replace('\\', "/");
    let mut current_target = normalized_target.clone();

    for _ in 0..10 {
        if let Some(&(entry_type, size, data_offset)) = entries_map.get(&current_target) {
            if entry_type == 1 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "대상 경로가 디렉토리입니다.",
                ));
            } else if entry_type == 0 {
                let mut data = archive_bytes[data_offset..data_offset + size].to_vec();
                if data.len() >= 4 && &data[0..3] == b"MZC" {
                    data = crate::decompress_bytes_v2_dict_password(&data, dict_data, password)
                        .map_err(|e| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("개별 파일 해제 실패: {:?}", e),
                            )
                        })?;
                }
                return Ok(data);
            } else if entry_type == 2 {
                let ref_bytes = &archive_bytes[data_offset..data_offset + size];
                let ref_str = std::str::from_utf8(ref_bytes)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
                current_target = ref_str.replace('\\', "/");
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("파일을 찾을 수 없습니다: {}", current_target),
            ));
        }
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "순환 참조 또는 너무 깊은 중복 참조 깊이입니다.",
    ))
}

/// **비솔리드(Non-Solid) MZAR 아카이브를 입력받아, 개별 압축된 모든 파일 엔트리를 해제하여 원본(Raw) MZAR 바이트 배열로 복구합니다.**
pub fn decompress_non_solid_archive(
    archive_bytes: &[u8],
    password: Option<&str>,
    dict_data: Option<&[u8]>,
) -> io::Result<Vec<u8>> {
    if archive_bytes.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "MZAR 데이터가 너무 짧습니다.",
        ));
    }
    if &archive_bytes[0..4] != MZAR_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "유효한 MZAR 아카이브가 아닙니다.",
        ));
    }

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
                "엔트리 헤더 읽기 실패",
            ));
        }
        let mut path_len_bytes = [0u8; 2];
        path_len_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 2]);
        let path_len = u16::from_le_bytes(path_len_bytes) as usize;
        cursor += 2;

        if cursor + path_len > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "상대 경로 읽기 실패",
            ));
        }
        let path_str = std::str::from_utf8(&archive_bytes[cursor..cursor + path_len])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
            .to_string();
        cursor += path_len;

        if cursor + 9 > data_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "플래그 읽기 실패",
            ));
        }
        let entry_type = archive_bytes[cursor];
        cursor += 1;

        let mut file_size_bytes = [0u8; 8];
        file_size_bytes.copy_from_slice(&archive_bytes[cursor..cursor + 8]);
        let file_size = u64::from_le_bytes(file_size_bytes) as usize;
        cursor += 8;

        if entry_type == 1 {
            entries.push(MzarEntry {
                relative_path: path_str,
                entry_type,
                size: 0,
                data: Vec::new(),
            });
        } else if entry_type == 0 {
            if cursor + file_size > data_len {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "파일 데이터가 잘렸습니다.",
                ));
            }
            let mut file_data = archive_bytes[cursor..cursor + file_size].to_vec();
            cursor += file_size;

            if file_data.len() >= 4 && &file_data[0..3] == b"MZC" {
                file_data =
                    crate::decompress_bytes_v2_dict_password(&file_data, dict_data, password)
                        .map_err(|e| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("개별 파일 해제 실패: {:?}", e),
                            )
                        })?;
            }

            entries.push(MzarEntry {
                relative_path: path_str,
                entry_type,
                size: file_data.len() as u64,
                data: file_data,
            });
        } else if entry_type == 2 {
            if cursor + file_size > data_len {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "중복 참조 경로가 잘렸습니다.",
                ));
            }
            let ref_bytes = &archive_bytes[cursor..cursor + file_size];
            cursor += file_size;
            entries.push(MzarEntry {
                relative_path: path_str,
                entry_type,
                size: file_size as u64,
                data: ref_bytes.to_vec(),
            });
        }
    }

    serialize_entries(entries)
}
