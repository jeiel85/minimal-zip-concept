use mzc::archive::{archive_directory, extract_archive, is_mzar_archive};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 테스트 전용 고유 임시 디렉토리를 생성합니다.
/// 타임스탬프와 테스트명 조합으로 충돌을 방지합니다.
fn create_unique_temp_dir(test_name: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mzar_test_{}_{}", test_name, timestamp));
    fs::create_dir_all(&dir).expect("임시 디렉토리 생성 실패");
    dir
}

/// 임시 디렉토리를 정리합니다. 이미 삭제된 경우 무시합니다.
fn cleanup_temp_dir(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

/// 두 디렉토리의 파일 구조와 내용이 동일한지 재귀적으로 검증합니다.
fn assert_dirs_equal(expected: &Path, actual: &Path) {
    let mut expected_entries: Vec<_> = collect_relative_entries(expected, expected);
    let mut actual_entries: Vec<_> = collect_relative_entries(actual, actual);

    expected_entries.sort();
    actual_entries.sort();

    assert_eq!(
        expected_entries.len(),
        actual_entries.len(),
        "엔트리 수 불일치: expected {} entries, got {}.\nExpected: {:?}\nActual: {:?}",
        expected_entries.len(),
        actual_entries.len(),
        expected_entries,
        actual_entries,
    );

    for (exp, act) in expected_entries.iter().zip(actual_entries.iter()) {
        assert_eq!(exp, act, "경로 불일치: expected {:?}, got {:?}", exp, act);

        let exp_path = expected.join(exp);
        let act_path = actual.join(act);

        if exp_path.is_file() {
            assert!(act_path.is_file(), "파일이 디렉토리로 복원됨: {:?}", act);
            let exp_content = fs::read(&exp_path).expect("원본 파일 읽기 실패");
            let act_content = fs::read(&act_path).expect("복원 파일 읽기 실패");
            assert_eq!(
                exp_content,
                act_content,
                "파일 내용 불일치: {:?} (expected {} bytes, got {} bytes)",
                exp,
                exp_content.len(),
                act_content.len(),
            );
        } else if exp_path.is_dir() {
            assert!(act_path.is_dir(), "디렉토리가 파일로 복원됨: {:?}", act);
        }
    }
}

/// 기준 디렉토리 기준 상대 경로 엔트리 목록을 수집합니다.
fn collect_relative_entries(base: &Path, current: &Path) -> Vec<String> {
    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(current) {
        for entry in read_dir {
            let entry = entry.expect("디렉토리 엔트리 읽기 실패");
            let path = entry.path();
            let relative = path
                .strip_prefix(base)
                .expect("상대 경로 계산 실패")
                .to_string_lossy()
                .replace('\\', "/");

            entries.push(relative);

            if path.is_dir() {
                entries.extend(collect_relative_entries(base, &path));
            }
        }
    }
    entries
}

// ================== 1. 파일 라운드트립 검증 ==================

#[test]
fn test_archive_roundtrip_with_files() {
    let src_dir = create_unique_temp_dir("roundtrip_files_src");
    let dest_dir = create_unique_temp_dir("roundtrip_files_dest");

    // 다양한 파일 생성
    fs::write(src_dir.join("hello.txt"), b"Hello, MZAR world!").unwrap();
    fs::write(src_dir.join("empty.dat"), b"").unwrap();
    fs::write(src_dir.join("binary.bin"), &[0x00, 0xFF, 0xAB, 0xCD, 0xEF]).unwrap();
    fs::write(
        src_dir.join("readme.md"),
        b"# MZAR Test\nThis is a test file for archive roundtrip.",
    )
    .unwrap();

    // 아카이브 생성 및 추출
    let archive_bytes = archive_directory(&src_dir).expect("아카이브 생성 실패");
    assert!(
        archive_bytes.len() > 8,
        "아카이브 바이트가 비정상적으로 작습니다."
    );

    extract_archive(&archive_bytes, &dest_dir, None, None).expect("아카이브 추출 실패");

    // 모든 파일 내용 일치 검증
    assert_dirs_equal(&src_dir, &dest_dir);

    // 개별 파일 내용 추가 확인
    assert_eq!(
        fs::read(dest_dir.join("hello.txt")).unwrap(),
        b"Hello, MZAR world!"
    );
    assert_eq!(fs::read(dest_dir.join("empty.dat")).unwrap(), b"");
    assert_eq!(
        fs::read(dest_dir.join("binary.bin")).unwrap(),
        &[0x00, 0xFF, 0xAB, 0xCD, 0xEF]
    );

    cleanup_temp_dir(&src_dir);
    cleanup_temp_dir(&dest_dir);
}

// ================== 2. 중첩 디렉토리 라운드트립 검증 ==================

#[test]
fn test_archive_roundtrip_nested_dirs() {
    let src_dir = create_unique_temp_dir("roundtrip_nested_src");
    let dest_dir = create_unique_temp_dir("roundtrip_nested_dest");

    // 중첩 디렉토리 구조 생성
    // src/
    //   level1/
    //     file1.txt
    //     level2/
    //       file2.txt
    //       level3/
    //         file3.txt
    //   another/
    //     data.bin
    let level1 = src_dir.join("level1");
    let level2 = level1.join("level2");
    let level3 = level2.join("level3");
    let another = src_dir.join("another");

    fs::create_dir_all(&level3).unwrap();
    fs::create_dir_all(&another).unwrap();

    fs::write(level1.join("file1.txt"), b"Level 1 content").unwrap();
    fs::write(level2.join("file2.txt"), b"Level 2 content - deeper").unwrap();
    fs::write(level3.join("file3.txt"), b"Level 3 - deepest file!").unwrap();
    fs::write(another.join("data.bin"), &[0x01, 0x02, 0x03, 0x04]).unwrap();

    // 루트 레벨 파일도 추가
    fs::write(src_dir.join("root.txt"), b"Root level file").unwrap();

    // 아카이브 생성 및 추출
    let archive_bytes = archive_directory(&src_dir).expect("중첩 디렉토리 아카이브 생성 실패");
    extract_archive(&archive_bytes, &dest_dir, None, None)
        .expect("중첩 디렉토리 아카이브 추출 실패");

    // 전체 구조 일치 검증
    assert_dirs_equal(&src_dir, &dest_dir);

    // 심층 파일 내용 확인
    assert_eq!(
        fs::read(
            dest_dir
                .join("level1")
                .join("level2")
                .join("level3")
                .join("file3.txt")
        )
        .unwrap(),
        b"Level 3 - deepest file!"
    );
    assert_eq!(
        fs::read(dest_dir.join("another").join("data.bin")).unwrap(),
        &[0x01, 0x02, 0x03, 0x04]
    );

    cleanup_temp_dir(&src_dir);
    cleanup_temp_dir(&dest_dir);
}

// ================== 3. 빈 디렉토리 라운드트립 검증 ==================

#[test]
fn test_archive_roundtrip_empty_dir() {
    let src_dir = create_unique_temp_dir("roundtrip_emptydir_src");
    let dest_dir = create_unique_temp_dir("roundtrip_emptydir_dest");

    // 빈 하위 디렉토리 생성
    let empty_sub = src_dir.join("empty_subdir");
    fs::create_dir_all(&empty_sub).unwrap();

    // 아카이브 생성 및 추출
    let archive_bytes = archive_directory(&src_dir).expect("빈 디렉토리 아카이브 생성 실패");
    extract_archive(&archive_bytes, &dest_dir, None, None).expect("빈 디렉토리 아카이브 추출 실패");

    // 빈 디렉토리 존재 여부 확인
    let restored_empty = dest_dir.join("empty_subdir");
    assert!(
        restored_empty.exists(),
        "빈 하위 디렉토리가 복원되지 않았습니다."
    );
    assert!(
        restored_empty.is_dir(),
        "빈 하위 디렉토리가 파일로 복원되었습니다."
    );

    // 디렉토리 내부가 비어있는지 확인
    let children: Vec<_> = fs::read_dir(&restored_empty)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        children.len(),
        0,
        "빈 디렉토리 안에 예상치 못한 엔트리가 있습니다."
    );

    cleanup_temp_dir(&src_dir);
    cleanup_temp_dir(&dest_dir);
}

// ================== 4. 대용량 파일(1MB) 라운드트립 검증 ==================

#[test]
fn test_archive_large_file() {
    let src_dir = create_unique_temp_dir("roundtrip_large_src");
    let dest_dir = create_unique_temp_dir("roundtrip_large_dest");

    // LCG 의사 난수 생성기로 1MB 데이터 생성 (외부 의존성 없이)
    let size = 1024 * 1024; // 1MB
    let mut data = Vec::with_capacity(size);
    let mut seed = 42u64;
    for _ in 0..size {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        data.push((seed >> 32) as u8);
    }

    fs::write(src_dir.join("large_random.bin"), &data).unwrap();

    // 아카이브 생성 및 추출
    let archive_bytes = archive_directory(&src_dir).expect("대용량 파일 아카이브 생성 실패");
    extract_archive(&archive_bytes, &dest_dir, None, None).expect("대용량 파일 아카이브 추출 실패");

    // 파일 내용 무결성 검증
    let restored_data = fs::read(dest_dir.join("large_random.bin")).unwrap();
    assert_eq!(
        data.len(),
        restored_data.len(),
        "대용량 파일 크기 불일치: expected {}, got {}",
        data.len(),
        restored_data.len(),
    );
    assert_eq!(data, restored_data, "대용량 파일(1MB) 바이트 내용 불일치");

    cleanup_temp_dir(&src_dir);
    cleanup_temp_dir(&dest_dir);
}

// ================== 5. is_mzar_archive 유효 데이터 검증 ==================

#[test]
fn test_is_mzar_archive_valid() {
    let src_dir = create_unique_temp_dir("is_mzar_valid");

    // 파일이 있는 디렉토리를 아카이브하여 실제 MZAR 바이트 생성
    fs::write(src_dir.join("test.txt"), b"test content").unwrap();
    let archive_bytes = archive_directory(&src_dir).expect("아카이브 생성 실패");

    assert!(
        is_mzar_archive(&archive_bytes),
        "유효한 MZAR 아카이브가 false로 판정되었습니다."
    );

    // 최소 유효 MZAR 헤더: magic(4) + entry_count(4) = 8바이트, 엔트리 0개
    let minimal_mzar = b"MZAR\x00\x00\x00\x00";
    assert!(
        is_mzar_archive(minimal_mzar),
        "최소 유효 MZAR 헤더가 false로 판정되었습니다."
    );

    // 엔트리 1개가 있는 수동 조립 MZAR
    let mut manual_mzar = Vec::new();
    manual_mzar.extend_from_slice(b"MZAR");
    manual_mzar.extend_from_slice(&1u32.to_le_bytes()); // entry count = 1
    let path_bytes = b"file.txt";
    manual_mzar.extend_from_slice(&(path_bytes.len() as u16).to_le_bytes());
    manual_mzar.extend_from_slice(path_bytes);
    manual_mzar.push(0); // is_dir = false
    manual_mzar.extend_from_slice(&5u64.to_le_bytes()); // file size = 5
    manual_mzar.extend_from_slice(b"hello"); // file data

    assert!(
        is_mzar_archive(&manual_mzar),
        "수동 조립 MZAR이 false로 판정되었습니다."
    );

    cleanup_temp_dir(&src_dir);
}

// ================== 6. is_mzar_archive 유효하지 않은 데이터 검증 ==================

#[test]
fn test_is_mzar_archive_invalid() {
    // 빈 바이트
    assert!(
        !is_mzar_archive(&[]),
        "빈 바이트가 유효한 MZAR로 판정되었습니다."
    );

    // 너무 짧은 바이트 (8바이트 미만)
    assert!(
        !is_mzar_archive(b"MZA"),
        "3바이트가 유효한 MZAR로 판정되었습니다."
    );
    assert!(
        !is_mzar_archive(b"MZAR"),
        "4바이트(매직만)가 유효한 MZAR로 판정되었습니다."
    );
    assert!(
        !is_mzar_archive(b"MZAR\x00"),
        "5바이트가 유효한 MZAR로 판정되었습니다."
    );

    // 잘못된 매직 바이트
    assert!(
        !is_mzar_archive(b"AAAA\x00\x00\x00\x00"),
        "잘못된 매직 바이트 AAAA가 유효로 판정되었습니다."
    );
    assert!(
        !is_mzar_archive(b"PK\x03\x04\x00\x00\x00\x00"),
        "ZIP 매직이 MZAR로 판정되었습니다."
    );
    assert!(
        !is_mzar_archive(b"\x1F\x8B\x08\x00\x00\x00\x00\x00"),
        "GZIP 매직이 MZAR로 판정되었습니다."
    );

    // 일반 텍스트
    assert!(
        !is_mzar_archive(b"Hello, world! This is not an archive."),
        "일반 텍스트가 MZAR로 판정되었습니다."
    );

    // 랜덤 바이너리
    assert!(
        !is_mzar_archive(&[0xFF, 0xFE, 0xFD, 0xFC, 0xFB, 0xFA, 0xF9, 0xF8]),
        "임의 바이너리가 MZAR로 판정되었습니다."
    );
}

// ================== 7. Zip-Slip 방어 검증 ==================

#[test]
fn test_zip_slip_defense() {
    let dest_dir = create_unique_temp_dir("zipslip_dest");

    // ../를 포함하는 경로 탈출 공격 MZAR 바이트를 수동 조립
    let mut malicious_mzar = Vec::new();
    malicious_mzar.extend_from_slice(b"MZAR");
    malicious_mzar.extend_from_slice(&1u32.to_le_bytes()); // entry count = 1

    let evil_path = b"../../../etc/evil.txt";
    malicious_mzar.extend_from_slice(&(evil_path.len() as u16).to_le_bytes());
    malicious_mzar.extend_from_slice(evil_path);
    malicious_mzar.push(0); // is_dir = false
    let evil_data = b"malicious payload";
    malicious_mzar.extend_from_slice(&(evil_data.len() as u64).to_le_bytes());
    malicious_mzar.extend_from_slice(evil_data);

    // 추출 시 PermissionDenied 에러가 발생해야 합니다
    let result = extract_archive(&malicious_mzar, &dest_dir, None, None);
    assert!(result.is_err(), "Zip-Slip 공격이 차단되지 않았습니다.");

    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        io::ErrorKind::PermissionDenied,
        "Zip-Slip 에러 종류가 PermissionDenied가 아닙니다: {:?}",
        err.kind(),
    );

    // 악의적 파일이 실제로 생성되지 않았는지 확인
    // (dest_dir 외부 경로에 파일이 없어야 함)
    let escaped_path = dest_dir
        .join("..")
        .join("..")
        .join("..")
        .join("etc")
        .join("evil.txt");
    assert!(
        !escaped_path.exists(),
        "Zip-Slip 공격으로 탈출 경로에 파일이 생성되었습니다: {:?}",
        escaped_path,
    );

    cleanup_temp_dir(&dest_dir);
}

// ================== 추가: Zip-Slip 변형 공격 방어 검증 ==================

#[test]
fn test_zip_slip_defense_absolute_path() {
    let dest_dir = create_unique_temp_dir("zipslip_abs_dest");

    // 절대 경로를 사용한 탈출 시도 (Windows/Unix 모두 대응)
    let mut malicious_mzar = Vec::new();
    malicious_mzar.extend_from_slice(b"MZAR");
    malicious_mzar.extend_from_slice(&1u32.to_le_bytes());

    // 상대 경로 상위 탈출 (다른 변형)
    let evil_path = b"subdir/../../escape.txt";
    malicious_mzar.extend_from_slice(&(evil_path.len() as u16).to_le_bytes());
    malicious_mzar.extend_from_slice(evil_path);
    malicious_mzar.push(0); // is_dir = false
    let evil_data = b"escape attempt";
    malicious_mzar.extend_from_slice(&(evil_data.len() as u64).to_le_bytes());
    malicious_mzar.extend_from_slice(evil_data);

    let result = extract_archive(&malicious_mzar, &dest_dir, None, None);
    assert!(
        result.is_err(),
        "상위 디렉토리 탈출 변형 공격이 차단되지 않았습니다."
    );

    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        io::ErrorKind::PermissionDenied,
        "Zip-Slip 변형 에러 종류가 PermissionDenied가 아닙니다: {:?}",
        err.kind(),
    );

    cleanup_temp_dir(&dest_dir);
}

// ================== 8. parse_mzar_metadata 검증 ==================

#[test]
fn test_parse_mzar_metadata() {
    let src_dir = create_unique_temp_dir("parse_metadata");

    // 파일 생성
    fs::write(src_dir.join("file1.txt"), b"some data").unwrap();
    fs::create_dir_all(src_dir.join("subdir")).unwrap();
    fs::write(src_dir.join("subdir").join("file2.dat"), b"more data").unwrap();

    let archive_bytes = archive_directory(&src_dir).expect("아카이브 생성 실패");

    let meta = mzc::archive::parse_mzar_metadata(&archive_bytes).expect("메타데이터 파싱 실패");

    assert_eq!(meta.len(), 3);

    // Sort metadata by relative path to make verification deterministic
    let mut sorted_meta = meta.clone();
    sorted_meta.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    assert_eq!(sorted_meta[0].relative_path, "file1.txt");
    assert_eq!(sorted_meta[0].is_dir, false);
    assert_eq!(sorted_meta[0].size, 9);

    assert_eq!(sorted_meta[1].relative_path, "subdir");
    assert_eq!(sorted_meta[1].is_dir, true);
    assert_eq!(sorted_meta[1].size, 0);

    assert_eq!(sorted_meta[2].relative_path, "subdir/file2.dat");
    assert_eq!(sorted_meta[2].is_dir, false);
    assert_eq!(sorted_meta[2].size, 9);

    cleanup_temp_dir(&src_dir);
}

#[test]
fn test_mzar_deduplication_and_parallel_extraction() {
    let src_dir = create_unique_temp_dir("dedup_parallel_src");
    let dest_dir = create_unique_temp_dir("dedup_parallel_dest");

    // Create duplicate file content
    let dup_content =
        b"This content is duplicated and should be deduplicated inside the MZAR archive.";
    fs::write(src_dir.join("file1.txt"), dup_content).unwrap();
    fs::write(src_dir.join("file2.txt"), dup_content).unwrap();

    // Create some subdirs
    fs::create_dir_all(src_dir.join("sub")).unwrap();
    fs::write(src_dir.join("sub").join("file3.txt"), dup_content).unwrap();

    // Create an unique file content
    fs::write(src_dir.join("unique.txt"), b"This content is unique!").unwrap();

    // Archive directory
    let archive_bytes = archive_directory(&src_dir).expect("아카이브 생성 실패");

    // Check metadata to verify entry types
    let meta = mzc::archive::parse_mzar_metadata(&archive_bytes).expect("메타데이터 파싱 실패");

    // There should be 5 entries: file1.txt, file2.txt, sub, sub/file3.txt, unique.txt
    assert_eq!(meta.len(), 5);

    // Verify duplicate entry type (entry_type == 2)
    // Find file2.txt and sub/file3.txt which should be duplicates of file1.txt (first occurrence)
    let mut file1 = None;
    let mut file2 = None;
    let mut file3 = None;
    let mut unique = None;
    let mut sub_dir = None;

    for m in &meta {
        if m.relative_path == "file1.txt" {
            file1 = Some(m.clone());
        } else if m.relative_path == "file2.txt" {
            file2 = Some(m.clone());
        } else if m.relative_path == "sub/file3.txt" {
            file3 = Some(m.clone());
        } else if m.relative_path == "unique.txt" {
            unique = Some(m.clone());
        } else if m.relative_path == "sub" {
            sub_dir = Some(m.clone());
        }
    }

    let file1 = file1.unwrap();
    let file2 = file2.unwrap();
    let file3 = file3.unwrap();
    let unique = unique.unwrap();
    let sub_dir = sub_dir.unwrap();

    // file1.txt should be standard file
    assert_eq!(file1.entry_type, 0);
    assert_eq!(file1.size, dup_content.len() as u64);

    // sub should be directory
    assert_eq!(sub_dir.entry_type, 1);

    // unique.txt should be standard file
    assert_eq!(unique.entry_type, 0);

    // file2.txt and sub/file3.txt should be duplicates (entry_type == 2)
    assert_eq!(file2.entry_type, 2);
    // Their size field stores the length of the reference path ("file1.txt" which is 9)
    assert_eq!(file2.size, 9);

    assert_eq!(file3.entry_type, 2);
    assert_eq!(file3.size, 9);

    // Now extract and verify roundtrip completeness!
    extract_archive(&archive_bytes, &dest_dir, None, None).expect("아카이브 추출 실패");

    assert_dirs_equal(&src_dir, &dest_dir);

    // Verify extract_single_file_from_mzar on both standard and duplicates
    let ext_unique =
        mzc::archive::extract_single_file_from_mzar(&archive_bytes, "unique.txt", None, None)
            .unwrap();
    assert_eq!(ext_unique, b"This content is unique!");

    let ext_dup =
        mzc::archive::extract_single_file_from_mzar(&archive_bytes, "sub/file3.txt", None, None)
            .unwrap();
    assert_eq!(ext_dup, dup_content);

    cleanup_temp_dir(&src_dir);
    cleanup_temp_dir(&dest_dir);
}

// ================== 9. 비솔리드 (Non-Solid) MZAR 라운드트립 검증 ==================

#[test]
fn test_mzar_non_solid_roundtrip() {
    use mzc::archive::{archive_directory_custom, decompress_non_solid_archive, CompressionParams};
    use mzc::cli::{CompressionMode, EntropyMode};

    let src_dir = create_unique_temp_dir("non_solid_roundtrip_src");
    let dest_dir = create_unique_temp_dir("non_solid_roundtrip_dest");

    // Create test files with various content
    fs::write(src_dir.join("hello.txt"), b"Hello, Non-Solid MZAR!").unwrap();
    fs::write(
        src_dir.join("data.bin"),
        &[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE],
    )
    .unwrap();
    fs::create_dir_all(src_dir.join("subdir")).unwrap();
    fs::write(
        src_dir.join("subdir").join("nested.txt"),
        b"Nested file content for non-solid test.",
    )
    .unwrap();

    // Create duplicate content to test dedup + non-solid interaction
    fs::write(src_dir.join("dup_a.txt"), b"Duplicate content here").unwrap();
    fs::write(src_dir.join("dup_b.txt"), b"Duplicate content here").unwrap();

    // Build compression params for non-solid mode (Hybrid + Huffman, level 1 for speed)
    let params = CompressionParams {
        mode: CompressionMode::Hybrid,
        entropy: EntropyMode::Huffman,
        level: 1,
        delta: false,
        bcj: false,
        png: false,
        lpc: false,
        bwt: false,
        dict_data: None,
        password: None,
        chunk_size: None,
        checksum_type: 0,
    };

    // Archive with non-solid compression (individual file compression)
    let non_solid_archive =
        archive_directory_custom(&src_dir, Some(&params)).expect("비솔리드 아카이브 생성 실패");

    // Verify it's a valid MZAR container
    assert!(
        is_mzar_archive(&non_solid_archive),
        "비솔리드 결과가 MZAR이 아닙니다."
    );

    // Verify metadata reports original (uncompressed) sizes for type-0 entries
    let meta = mzc::archive::parse_mzar_metadata(&non_solid_archive)
        .expect("비솔리드 메타데이터 파싱 실패");
    let hello_meta = meta
        .iter()
        .find(|m| m.relative_path == "hello.txt")
        .unwrap();
    assert_eq!(hello_meta.entry_type, 0);
    // parse_mzar_metadata should resolve to original size via MzcHeader
    assert_eq!(
        hello_meta.size,
        b"Hello, Non-Solid MZAR!".len() as u64,
        "비솔리드 메타데이터의 원본 크기가 올바르지 않습니다."
    );

    // Extract the non-solid archive directly
    extract_archive(&non_solid_archive, &dest_dir, None, None)
        .expect("비솔리드 아카이브 추출 실패");

    // Verify full roundtrip correctness
    assert_dirs_equal(&src_dir, &dest_dir);

    // Verify individual file content
    assert_eq!(
        fs::read(dest_dir.join("hello.txt")).unwrap(),
        b"Hello, Non-Solid MZAR!"
    );
    assert_eq!(
        fs::read(dest_dir.join("data.bin")).unwrap(),
        &[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE]
    );
    assert_eq!(
        fs::read(dest_dir.join("subdir").join("nested.txt")).unwrap(),
        b"Nested file content for non-solid test."
    );
    assert_eq!(
        fs::read(dest_dir.join("dup_a.txt")).unwrap(),
        b"Duplicate content here"
    );
    assert_eq!(
        fs::read(dest_dir.join("dup_b.txt")).unwrap(),
        b"Duplicate content here"
    );

    // Verify single-file extraction from non-solid archive
    let single = mzc::archive::extract_single_file_from_mzar(
        &non_solid_archive,
        "subdir/nested.txt",
        None,
        None,
    )
    .unwrap();
    assert_eq!(single, b"Nested file content for non-solid test.");

    // Verify decompress_non_solid_archive produces a valid raw MZAR
    let raw_mzar = decompress_non_solid_archive(&non_solid_archive, None, None)
        .expect("비솔리드 아카이브 디코딩 실패");
    assert!(
        is_mzar_archive(&raw_mzar),
        "디코딩된 Raw MZAR이 유효하지 않습니다."
    );

    // The raw MZAR should also extract correctly
    let dest_dir2 = create_unique_temp_dir("non_solid_roundtrip_dest2");
    extract_archive(&raw_mzar, &dest_dir2, None, None).expect("Raw MZAR 아카이브 추출 실패");
    assert_dirs_equal(&src_dir, &dest_dir2);

    cleanup_temp_dir(&src_dir);
    cleanup_temp_dir(&dest_dir);
    cleanup_temp_dir(&dest_dir2);
}

// ================== 10. MZC9 Configurable Chunk Size & Checksum Type Roundtrip ==================

#[test]
fn test_mzar_mzc9_configurable_chunk_roundtrip() {
    use mzc::archive::{archive_directory_custom, CompressionParams};
    use mzc::cli::{CompressionMode, EntropyMode};

    let src_dir = create_unique_temp_dir("mzc9_chunk_src");
    let dest_dir = create_unique_temp_dir("mzc9_chunk_dest");

    // Create a moderately sized file to split across chunks
    let mut large_content = Vec::new();
    for i in 0..5000 {
        large_content.extend_from_slice(format!("Line {} of data repeat. ", i).as_bytes());
    }
    fs::write(src_dir.join("large.txt"), &large_content).unwrap();

    // Compression Params with custom chunk size (16KB = 16384 bytes) and CRC-32 (checksum_type = 1)
    let params = CompressionParams {
        mode: CompressionMode::Hybrid,
        entropy: EntropyMode::Huffman,
        level: 4,
        delta: false,
        bcj: false,
        png: false,
        lpc: false,
        bwt: false,
        dict_data: None,
        password: None,
        chunk_size: Some(16384),
        checksum_type: 1, // CRC-32
    };

    let archive_bytes =
        archive_directory_custom(&src_dir, Some(&params)).expect("MZC9 아카이브 생성 실패");

    extract_archive(&archive_bytes, &dest_dir, None, None).expect("MZC9 아카이브 추출 실패");

    assert_dirs_equal(&src_dir, &dest_dir);

    cleanup_temp_dir(&src_dir);
    cleanup_temp_dir(&dest_dir);
}

#[test]
fn test_mzar_crc32_roundtrip() {
    use mzc::archive::{archive_directory_custom, CompressionParams};
    use mzc::cli::{CompressionMode, EntropyMode};

    let src_dir = create_unique_temp_dir("mzc9_crc32_src");
    let dest_dir = create_unique_temp_dir("mzc9_crc32_dest");

    fs::write(src_dir.join("hello.txt"), b"CRC32 Checksum Test Content").unwrap();

    let params = CompressionParams {
        mode: CompressionMode::Rle,
        entropy: EntropyMode::None,
        level: 1,
        delta: false,
        bcj: false,
        png: false,
        lpc: false,
        bwt: false,
        dict_data: None,
        password: None,
        chunk_size: None,
        checksum_type: 1, // CRC-32
    };

    let archive_bytes =
        archive_directory_custom(&src_dir, Some(&params)).expect("MZC9 CRC32 아카이브 생성 실패");

    extract_archive(&archive_bytes, &dest_dir, None, None).expect("MZC9 CRC32 아카이브 추출 실패");

    assert_dirs_equal(&src_dir, &dest_dir);

    cleanup_temp_dir(&src_dir);
    cleanup_temp_dir(&dest_dir);
}

// ================== 11. Archive Recovery Tool Verification ==================

#[test]
fn test_archive_recovery() {
    use mzc::archive::{archive_directory_custom, CompressionParams};
    use mzc::cli::{CompressionMode, EntropyMode};
    use mzc::recover::recover_bytes;

    let src_dir = create_unique_temp_dir("recovery_src");

    let file_content = b"This is a recovery test file. We want to see if we can recover this even if the archive is truncated!";
    fs::write(src_dir.join("recover_me.txt"), file_content).unwrap();

    // Create a normal archive
    let params = CompressionParams {
        mode: CompressionMode::Hybrid,
        entropy: EntropyMode::Huffman,
        level: 3,
        delta: false,
        bcj: false,
        png: false,
        lpc: false,
        bwt: false,
        dict_data: None,
        password: None,
        chunk_size: None,
        checksum_type: 0,
    };
    let archive_bytes = archive_directory_custom(&src_dir, Some(&params)).unwrap();

    // Artificially truncate the archive bytes (keep first 80%)
    let truncate_len = (archive_bytes.len() * 80) / 100;
    let truncated_bytes = &archive_bytes[0..truncate_len];

    // Attempt recovery on truncated bytes
    let recovered = recover_bytes(truncated_bytes);

    if let Ok(entries) = recovered {
        for (path, data) in entries {
            if path.contains("recover_me.txt") {
                assert_eq!(data, file_content);
                cleanup_temp_dir(&src_dir);
                return;
            }
        }
    }

    cleanup_temp_dir(&src_dir);
}

#[test]
fn test_fuzz_mzar_overflow_regression() {
    let crash_input = [
        0x4d, 0x5a, 0x41, 0x52, 0xf6, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00,
        0xa3, 0x00, 0x00, 0x00,
    ];

    let result = std::panic::catch_unwind(|| mzc::decompress_bytes_v2(&crash_input));
    assert!(result.is_ok(), "malformed MZAR input must not panic");
    assert!(
        result.unwrap().is_err(),
        "malformed MZAR input should be rejected"
    );
}
