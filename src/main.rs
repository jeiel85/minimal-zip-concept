mod cli;

use std::fs;
use anyhow::{Context, Result};
use clap::Parser;

// mzc 라이브러리의 통합 압축/해제 파이프라인과 서브커맨드 인프라를 활용합니다.
use mzc::cli::{Cli, Commands};
use mzc::checksum::{calculate_sha256, bytes_to_hex};
use mzc::inspect::inspect_mzc_file;
use mzc::{compress_bytes_v2, decompress_bytes_v2};

/// MZC CLI 엔트리포인트입니다.
/// 모든 실제 병렬화 및 청크 압축해제 기능은 mzc 라이브러리 모듈에서 제공받아 호출합니다.
fn main() -> Result<()> {
    // 1. CLI 명령줄 인자를 파싱합니다.
    let cli = Cli::parse();

    // 2. 입력받은 서브커맨드에 따라 분기 처리합니다.
    match cli.command {
        Commands::Compress { input_file, output_file, mode, entropy, level, delta, bcj } => {
            println!("압축 시작: {:?} -> {:?}", input_file, output_file);
            println!("알고리즘 모드: {:?}, 엔트로피 코딩: {:?}, 레벨: {}, 델타 필터: {}, BCJ 필터: {}", mode, entropy, level, delta, bcj);
            
            // 원본 파일 바이트 로드
            let original_bytes = fs::read(&input_file)
                .with_context(|| format!("원본 파일 '{:?}'을 읽을 수 없습니다.", input_file))?;
            
            let original_size = original_bytes.len() as u64;

            // 고도화된 MZC2/MZC5 병렬 청크 및 허프만 파이프라인 구동
            let final_output = compress_bytes_v2(&original_bytes, mode, entropy, level, delta, bcj);

            // 파일에 기록
            fs::write(&output_file, &final_output)
                .with_context(|| format!("압축 파일 '{:?}'을 저장하는 데 실패했습니다.", output_file))?;

            // 메타데이터 요약 출력
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

        Commands::Decompress { input_file, output_file } => {
            println!("압축 해제 시작: {:?} -> {:?}", input_file, output_file);

            // 압축 데이터 전체 로드
            let compressed_bytes = fs::read(&input_file)
                .with_context(|| format!("압축 파일 '{:?}'을 읽을 수 없습니다.", input_file))?;

            // 라이브러리의 검증 포함 통합 병렬 청크 압축 해제 파이프라인 구동
            let decompressed_bytes = decompress_bytes_v2(&compressed_bytes)
                .context("MZC 압축 파일 해제 및 검증 과정에서 오류가 발생했습니다.")?;

            // 복원 파일 저장
            fs::write(&output_file, &decompressed_bytes)
                .with_context(|| format!("복원 파일 '{:?}'을 저장하는 데 실패했습니다.", output_file))?;

            let restored_hash_hex = bytes_to_hex(&calculate_sha256(&decompressed_bytes));

            println!("압축 해제 및 검증 완료!");
            println!("Restored size: {} bytes", decompressed_bytes.len());
            println!("SHA-256: {}", restored_hash_hex);
            println!("Verified: OK");
        }

        Commands::Test { input_file, mode, entropy, level, delta, bcj } => {
            println!("라운드트립 자가 검증 테스트 시작: {:?}", input_file);
            println!("테스트 알고리즘 모드: {:?}, 엔트로피 코딩: {:?}, 레벨: {}, 델타 필터: {}, BCJ 필터: {}", mode, entropy, level, delta, bcj);

            // 원본 파일 바이트 로드
            let original_bytes = fs::read(&input_file)
                .with_context(|| format!("테스트 파일 '{:?}'을 읽을 수 없습니다.", input_file))?;

            let original_size = original_bytes.len() as u64;
            let sha256_hex = bytes_to_hex(&calculate_sha256(&original_bytes));

            // 1. 메모리 압축
            let compressed_bytes = compress_bytes_v2(&original_bytes, mode, entropy, level, delta, bcj);
            let total_compressed_size = compressed_bytes.len();

            // 2. 메모리 해제 및 체크섬 검증
            let decompressed_bytes = decompress_bytes_v2(&compressed_bytes)
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

            // 최종 교차 안전망 확인
            assert_eq!(original_bytes, decompressed_bytes);
        }

        Commands::Inspect { input_file } => {
            inspect_mzc_file(&input_file)?;
        }

        Commands::Gui => {
            println!("MZC 데스크톱 GUI 애플리케이션을 시작합니다...");
            mzc::gui::run_gui_app().map_err(|e| anyhow::anyhow!("GUI 앱 구동 실패: {}", e))?;
        }
    }

    Ok(())
}
