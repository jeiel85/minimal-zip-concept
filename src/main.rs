mod cli;

use std::fs;
use anyhow::{Context, Result};
use clap::Parser;

// mzc 라이브러리의 통합 압축/해제 파이프라인과 서브커맨드 인프라를 활용합니다.
// # Rust 개념 설명:
// - `mzc::cli::*`: mzc 라이브러리에 선언된 cli 모듈의 유용한 타입들을 가져옵니다.
use mzc::cli::{Cli, Commands};
use mzc::checksum::{calculate_sha256, bytes_to_hex};
use mzc::inspect::inspect_mzc_file;

/// **MZC CLI 엔트리포인트 (메인 함수)**
///
/// # Rust 개념 설명:
/// - `fn main() -> Result<()>`: 메인 함수는 에러가 발생할 수 있는 `Result` 타입을 반환합니다.
///   성공 시 `Ok(())`를, 실패 시 `Err(오류내용)`를 반환하며, CLI 구동 중 생기는 모든 오류는 자동으로 포착되어 터미널에 에러 로그로 표출됩니다.
fn main() -> Result<()> {
    // 1. CLI 명령줄 인자를 자동으로 분석하고 파싱합니다.
    let cli = Cli::parse();

    // 2. 입력받은 서브커맨드(Commands)에 맞춰 패턴 매칭 분기를 수행합니다.
    match cli.command {
        // --- 압축 (Compress) 서브커맨드 실행 분기 ---
        Commands::Compress { 
            input_file, 
            output_file, 
            mode, 
            entropy, 
            level, 
            delta, 
            bcj, 
            png, 
            lpc, 
            dict_file 
        } => {
            println!("압축 시작: {:?} -> {:?}", input_file, output_file);
            println!("알고리즘 모드: {:?}, 엔트로피 코딩: {:?}, 레벨: {}, 델타 필터: {}, BCJ 필터: {}, PNG 필터: {}, LPC 필터: {}", 
                     mode, entropy, level, delta, bcj, png, lpc);
            if let Some(ref path) = dict_file {
                println!("사용할 사전 파일: {:?}", path);
            }
            
            // 원본 파일의 원시 바이트를 로드합니다.
            // `.with_context`는 에러 발생 시 부가 정보를 덧붙여 에러를 추적하기 쉽게 만들어 줍니다.
            let original_bytes = fs::read(&input_file)
                .with_context(|| format!("원본 파일 '{:?}'을 읽을 수 없습니다.", input_file))?;
            
            let original_size = original_bytes.len() as u64;

            // 만약 사전 파일 경로가 주어졌다면, 바이트 데이터를 로드합니다.
            let dict_bytes = if let Some(ref path) = dict_file {
                let bytes = fs::read(path)
                    .with_context(|| format!("사전 파일 '{:?}'을 읽을 수 없습니다.", path))?;
                Some(bytes)
            } else {
                None
            };

            // 고도화된 MZC 병렬 청크 압축 파이프라인 구동 (MZC7 기능 포함)
            let final_output = mzc::compress_bytes_v2_dict(
                &original_bytes,
                mode,
                entropy,
                level,
                delta,
                bcj,
                png,
                lpc,
                dict_bytes.as_deref(),
            );

            // 최종 압축 파일 디스크에 저장
            fs::write(&output_file, &final_output)
                .with_context(|| format!("압축 파일 '{:?}'을 저장하는 데 실패했습니다.", output_file))?;

            // 압축 성능 보고
            let total_compressed_size = final_output.len();
            let ratio = if original_size > 0 {
                (total_compressed_size as f64 / original_size as f64) * 100.0
            } else {
                100.0
            };
            let sha256_hex = bytes_to_hex(&calculate_sha256(&original_bytes));

            println!("압축 완료!");
            println!("Original size: {} bytes", original_size);
            println!("Compressed size: {} bytes", total_compressed_size);
            println!("Ratio: {:.2}%", ratio);
            println!("SHA-256: {}", sha256_hex);
        }

        // --- 압축 해제 (Decompress) 서브커맨드 실행 분기 ---
        Commands::Decompress { input_file, output_file, dict_file } => {
            println!("압축 해제 시작: {:?} -> {:?}", input_file, output_file);
            if let Some(ref path) = dict_file {
                println!("사용할 사전 파일: {:?}", path);
            }

            // 압축 데이터 로드
            let compressed_bytes = fs::read(&input_file)
                .with_context(|| format!("압축 파일 '{:?}'을 읽을 수 없습니다.", input_file))?;

            // 사전 데이터 로드
            let dict_bytes = if let Some(ref path) = dict_file {
                let bytes = fs::read(path)
                    .with_context(|| format!("사전 파일 '{:?}'을 읽을 수 없습니다.", path))?;
                Some(bytes)
            } else {
                None
            };

            // 라이브러리의 검증 포함 통합 병렬 청크 압축 해제 파이프라인 구동
            let decompressed_bytes = mzc::decompress_bytes_v2_dict(&compressed_bytes, dict_bytes.as_deref())
                .context("MZC 압축 파일 해제 및 검증 과정에서 오류가 발생했습니다.")?;

            // 복원된 파일 디스크 저장
            fs::write(&output_file, &decompressed_bytes)
                .with_context(|| format!("복원 파일 '{:?}'을 저장하는 데 실패했습니다.", output_file))?;

            let restored_hash_hex = bytes_to_hex(&calculate_sha256(&decompressed_bytes));

            println!("압축 해제 및 검증 완료!");
            println!("Restored size: {} bytes", decompressed_bytes.len());
            println!("SHA-256: {}", restored_hash_hex);
            println!("Verified: OK");
        }

        // --- 라운드트립 검증 테스트 (Test) 서브커맨드 실행 분기 ---
        Commands::Test { 
            input_file, 
            mode, 
            entropy, 
            level, 
            delta, 
            bcj, 
            png, 
            lpc, 
            dict_file 
        } => {
            println!("라운드트립 자가 검증 테스트 시작: {:?}", input_file);
            println!("테스트 알고리즘 모드: {:?}, 엔트로피 코딩: {:?}, 레벨: {}, 델타 필터: {}, BCJ 필터: {}, PNG 필터: {}, LPC 필터: {}", 
                     mode, entropy, level, delta, bcj, png, lpc);
            if let Some(ref path) = dict_file {
                println!("사용할 사전 파일: {:?}", path);
            }

            let original_bytes = fs::read(&input_file)
                .with_context(|| format!("테스트 파일 '{:?}'을 읽을 수 없습니다.", input_file))?;

            let original_size = original_bytes.len() as u64;
            let sha256_hex = bytes_to_hex(&calculate_sha256(&original_bytes));

            let dict_bytes = if let Some(ref path) = dict_file {
                let bytes = fs::read(path)
                    .with_context(|| format!("사전 파일 '{:?}'을 읽을 수 없습니다.", path))?;
                Some(bytes)
            } else {
                None
            };

            // 1. 메모리상에서 즉각 압축
            let compressed_bytes = mzc::compress_bytes_v2_dict(
                &original_bytes,
                mode,
                entropy,
                level,
                delta,
                bcj,
                png,
                lpc,
                dict_bytes.as_deref(),
            );
            let total_compressed_size = compressed_bytes.len();

            // 2. 메모리상에서 즉각 해제 및 체크섬 검증
            let decompressed_bytes = mzc::decompress_bytes_v2_dict(&compressed_bytes, dict_bytes.as_deref())
                .context("인메모리 자가 해제 검증 중 오류가 발생했습니다.")?;

            let ratio = if original_size > 0 {
                (total_compressed_size as f64 / original_size as f64) * 100.0
            } else {
                100.0
            };

            println!("Original size: {} bytes", original_size);
            println!("Compressed size: {} bytes", total_compressed_size);
            println!("Ratio: {:.2}%", ratio);
            println!("SHA-256: {}", sha256_hex);
            println!("Verified: OK");

            // Assert 단언을 통해 원본 바이트와 복원 바이트가 완전히 하나도 빠짐없이 완벽히 대칭되는지 체크합니다.
            assert_eq!(original_bytes, decompressed_bytes);
        }

        // --- 공유 사전 생성 학습 (Train) 서브커맨드 실행 분기 ---
        Commands::Train { input_files, output } => {
            println!("사전 학습 시작 (총 {}개 파일)...", input_files.len());
            
            let mut all_bytes = Vec::new();
            for file_path in &input_files {
                println!("학습 대상 파일 로드: {:?}", file_path);
                let bytes = fs::read(file_path)
                    .with_context(|| format!("학습용 파일 '{:?}'을 읽을 수 없습니다.", file_path))?;
                all_bytes.extend_from_slice(&bytes);
            }

            println!("해시 테이블 및 가중치 빈도 스캔을 통한 패턴 사전 추출 중...");
            let dict = mzc::rle::build_dictionary(&all_bytes);
            let dict_bytes = dict.to_bytes();

            fs::write(&output, &dict_bytes)
                .with_context(|| format!("학습된 사전 파일 '{:?}'을 저장하는 데 실패했습니다.", output))?;

            println!("사전 학습 완료!");
            println!("추출된 사전 단어 개수: {}", dict.entries.len());
            println!("사전 파일 저장 경로: {:?}", output);
            println!("사전 바이트 크기: {} bytes", dict_bytes.len());
        }

        // --- 압축 파일 헤더 디테일 분석 (Inspect) 서브커맨드 실행 분기 ---
        Commands::Inspect { input_file } => {
            inspect_mzc_file(&input_file)?;
        }

        // --- 표준 GZIP / raw DEFLATE 디코더 해제 (Inflate) 서브커맨드 실행 분기 ---
        Commands::Inflate { input_file, output_file } => {
            println!("Inflate 해제 구동: {:?} -> {:?}", input_file, output_file);
            
            let input_bytes = fs::read(&input_file)
                .with_context(|| format!("입력 파일 '{:?}'을 읽을 수 없습니다.", input_file))?;

            // GZIP 매직 2바이트 `1F 8B` 시작 여부에 따라 적절한 디코더 라이브러리를 바인딩합니다.
            let decompressed = if input_bytes.starts_with(&[0x1F, 0x8B]) {
                println!("GZIP 헤더 포맷 감지. RFC 1952 스펙에 맞춰 GZIP 체크섬 및 페이로드를 해제합니다.");
                mzc::deflate::gzip_decompress(&input_bytes)
                    .context("GZIP 압축 해제 오류 발생")?
            } else {
                println!("raw DEFLATE 비트스트림 감지. RFC 1951 스펙에 맞춰 동적 허프만 트리를 해제합니다.");
                mzc::deflate::inflate(&input_bytes)
                    .context("raw DEFLATE 압축 해제 오류 발생")?
            };

            fs::write(&output_file, &decompressed)
                .with_context(|| format!("복원 파일 '{:?}'을 저장하는 데 실패했습니다.", output_file))?;

            println!("압축 해제 및 복구 완료! 복원 파일 크기: {} bytes", decompressed.len());
        }

        // --- 데스크톱 그래픽 GUI 애플리케이션 실행 분기 ---
        Commands::Gui => {
            println!("MZC 데스크톱 GUI 애플리케이션을 시작합니다...");
            mzc::gui::run_gui_app().map_err(|e| anyhow::anyhow!("GUI 앱 구동 실패: {}", e))?;
        }
    }

    Ok(())
}
