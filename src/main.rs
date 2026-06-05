mod cli;

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;

// mzc 라이브러리의 통합 압축/해제 파이프라인과 서브커맨드 인프라를 활용합니다.
// # Rust 개념 설명:
// - `mzc::cli::*`: mzc 라이브러리에 선언된 cli 모듈의 유용한 타입들을 가져옵니다.
use mzc::checksum::{bytes_to_hex, calculate_sha256};
use mzc::cli::{Cli, Commands};
use mzc::inspect::inspect_mzc_file;

/// **MZC CLI 엔트리포인트 (메인 함수)**
///
/// # Rust 개념 설명:
/// - `fn main() -> Result<()>`: 메인 함수는 에러가 발생할 수 있는 `Result` 타입을 반환합니다.
///   성공 시 `Ok(())`를, 실패 시 `Err(오류내용)`를 반환하며, CLI 구동 중 생기는 모든 오류는 자동으로 포착되어 터미널에 에러 로그로 표출됩니다.
fn main() -> Result<()> {
    // 1. CLI 명령줄 인자를 자동으로 분석하고 파싱합니다.
    // 만약 단일 파일/폴더 경로가 입력되었을 경우 서브커맨드(compress/decompress)를 자동으로 보정하여 제공합니다.
    let mut args: Vec<String> = std::env::args().collect();
    if args.len() == 2 {
        let first_arg = &args[1];
        let subcommands = [
            "compress",
            "decompress",
            "test",
            "train",
            "inspect",
            "inflate",
            "bench",
            "gui",
            "register-context-menu",
            "unregister-context-menu",
            "-h",
            "--help",
            "-V",
            "--version",
        ];
        if !subcommands.contains(&first_arg.as_str()) {
            let path = std::path::Path::new(first_arg);
            if path.exists() {
                if path.extension().map_or(false, |ext| ext == "mzc") {
                    args.insert(1, "decompress".to_string());
                } else {
                    args.insert(1, "compress".to_string());
                }
            }
        }
    }
    let cli = Cli::parse_from(args);

    // 2. 입력받은 서브커맨드(Commands)에 맞춰 패턴 매칭 분기를 수행합니다.
    match cli.command {
        // --- 압축 (Compress) 서브커맨드 실행 분기 ---
        Commands::Compress {
            input_paths,
            output_file,
            mode,
            entropy,
            level,
            delta,
            bcj,
            png,
            lpc,
            bwt,
            dict_file,
            password,
        } => {
            // 출력 파일 경로 자동 추론 (첫 번째 입력 경로 기준)
            let out_file = match output_file {
                Some(path) => path,
                None => {
                    let mut path = input_paths[0].clone();
                    if let Some(ext) = path.extension() {
                        let mut new_ext = ext.to_os_string();
                        new_ext.push(".mzc");
                        path.set_extension(new_ext);
                    } else {
                        path.set_extension("mzc");
                    }
                    path
                }
            };

            println!("압축 시작: {:?} -> {:?}", input_paths, out_file);
            println!("알고리즘 모드: {:?}, 엔트로피 코딩: {:?}, 레벨: {}, 델타 필터: {}, BCJ 필터: {}, PNG 필터: {}, LPC 필터: {}, BWT 필터: {}",
                     mode, entropy, level, delta, bcj, png, lpc, bwt);
            if let Some(ref path) = dict_file {
                println!("사용할 사전 파일: {:?}", path);
            }
            if password.is_some() {
                println!("비밀번호 기반 암호화(AES-256)가 설정되었습니다.");
            }

            // 원본 파일의 원시 바이트를 로드합니다.
            // 입력 경로가 여러 개이거나, 디렉토리인 경우 MZAR 아카이브 컨테이너로 먼저 패킹합니다.
            let is_multi_or_dir = input_paths.len() > 1 || input_paths[0].is_dir();
            let original_bytes = if is_multi_or_dir {
                println!("여러 파일 또는 디렉토리가 감지되었습니다. MZAR 아카이브로 패킹을 먼저 진행합니다.");
                mzc::archive::archive_paths(&input_paths)
                    .with_context(|| format!("입력 파일/디렉토리 아카이빙 실패: {:?}", input_paths))?
            } else {
                fs::read(&input_paths[0])
                    .with_context(|| format!("원본 파일 '{:?}'을 읽을 수 없습니다.", input_paths[0]))?
            };

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
            let final_output = if original_bytes.len() > 100 * 1024 {
                use indicatif::{ProgressBar, ProgressStyle};
                let chunk_size = 1024 * 1024; // 1MB chunks
                let total_chunks = (original_bytes.len() + chunk_size - 1) / chunk_size;

                println!("대용량 파일 압축 중... (총 {}개 청크)", total_chunks);
                let pb = ProgressBar::new(total_chunks as u64);
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} 청크 ({eta})")
                        .unwrap()
                        .progress_chars("#>-")
                );

                let pb_clone = pb.clone();
                let result = mzc::compress_bytes_v2_with_progress_dict_password(
                    &original_bytes,
                    mode,
                    entropy,
                    level,
                    delta,
                    bcj,
                    png,
                    lpc,
                    bwt,
                    dict_bytes.as_deref(),
                    password.as_deref(),
                    move |chunk_idx, _total, _, _| {
                        pb_clone.set_position(chunk_idx as u64);
                    },
                );
                pb.finish_with_message("압축 완료");
                result
            } else {
                mzc::compress_bytes_v2_with_progress_dict_password(
                    &original_bytes,
                    mode,
                    entropy,
                    level,
                    delta,
                    bcj,
                    png,
                    lpc,
                    bwt,
                    dict_bytes.as_deref(),
                    password.as_deref(),
                    |_, _, _, _| {},
                )
            };

            // 최종 압축 파일 디스크에 저장
            fs::write(&out_file, &final_output).with_context(|| {
                format!("압축 파일 '{:?}'을 저장하는 데 실패했습니다.", out_file)
            })?;

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
        Commands::Decompress {
            input_file,
            output_file,
            dict_file,
            password,
        } => {
            // 출력 파일 경로 자동 추론
            let out_file = match output_file {
                Some(path) => path,
                None => {
                    let mut path = input_file.clone();
                    if path.extension().map_or(false, |ext| ext == "mzc") {
                        path.set_extension(""); // Remove .mzc extension
                    } else {
                        let mut new_ext = path.extension().unwrap_or_default().to_os_string();
                        new_ext.push(".extracted");
                        path.set_extension(new_ext);
                    }
                    path
                }
            };

            println!("압축 해제 시작: {:?} -> {:?}", input_file, out_file);
            if let Some(ref path) = dict_file {
                println!("사용할 사전 파일: {:?}", path);
            }

            // 압축 데이터 로드
            let compressed_bytes = fs::read(&input_file)
                .with_context(|| format!("압축 파일 '{:?}'을 읽을 수 없습니다.", input_file))?;

            // 대화형 비밀번호 프로그래밍: 파일이 MZC8(암호화 버전)이고 CLI 비밀번호가 없는 경우 사용자에게 보안 프롬프트를 띄웁니다.
            let mut final_password = password;
            if final_password.is_none() {
                if let Ok(header) = mzc::format::MzcHeader::from_bytes(&compressed_bytes) {
                    if header.version == mzc::format::VERSION_MZC8 {
                        println!("이 압축 파일은 암호화(AES-256)되어 있습니다.");
                        let prompt_pass = rpassword::prompt_password("비밀번호를 입력해 주세요: ")
                            .with_context(|| "비밀번호 입력 과정에서 실패했습니다.")?;
                        final_password = Some(prompt_pass);
                    }
                }
            }

            // 사전 데이터 로드
            let dict_bytes = if let Some(ref path) = dict_file {
                let bytes = fs::read(path)
                    .with_context(|| format!("사전 파일 '{:?}'을 읽을 수 없습니다.", path))?;
                Some(bytes)
            } else {
                None
            };

            // 라이브러리의 검증 포함 통합 병렬 청크 압축 해제 파이프라인 구동
            let decompressed_bytes = if compressed_bytes.len() > 100 * 1024 {
                use indicatif::{ProgressBar, ProgressStyle};

                let pb = ProgressBar::new(100);
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} 청크 ({eta})")
                        .unwrap()
                        .progress_chars("#>-")
                );

                let pb_clone = pb.clone();
                let result = mzc::decompress_bytes_v2_with_progress_dict_password(
                    &compressed_bytes,
                    dict_bytes.as_deref(),
                    final_password.as_deref(),
                    move |chunk_idx, total_chunks| {
                        pb_clone.set_length(total_chunks as u64);
                        pb_clone.set_position(chunk_idx as u64);
                    }
                );
                pb.finish_with_message("해제 완료");
                result
            } else {
                mzc::decompress_bytes_v2_dict_password(&compressed_bytes, dict_bytes.as_deref(), final_password.as_deref())
            }.context("MZC 압축 파일 해제 및 검증 과정에서 오류가 발생했습니다.")?;

            // 복원된 데이터가 MZAR 컨테이너 아카이브인지 감지
            if mzc::archive::is_mzar_archive(&decompressed_bytes) {
                println!("복원된 바이트에서 MZAR 컨테이너 헤더가 감지되었습니다. 폴더 구조 추출을 시작합니다.");
                mzc::archive::extract_archive(&decompressed_bytes, &out_file).with_context(
                    || format!("디렉토리 추출에 실패했습니다. 타겟 경로: {:?}", out_file),
                )?;
                println!("디렉토리 아카이브 복원 성공!");
            } else {
                fs::write(&out_file, &decompressed_bytes).with_context(|| {
                    format!("복원 파일 '{:?}'을 저장하는 데 실패했습니다.", out_file)
                })?;
            }

            let restored_hash_hex = bytes_to_hex(&calculate_sha256(&decompressed_bytes));

            println!("압축 해제 및 검증 완료!");
            println!("Restored size: {} bytes", decompressed_bytes.len());
            println!("SHA-256: {}", restored_hash_hex);
            println!("Verified: OK");
        }

        // --- 라운드트립 검증 테스트 (Test) 서브커맨드 실행 분기 ---
        Commands::Test {
            input_paths,
            mode,
            entropy,
            level,
            delta,
            bcj,
            png,
            lpc,
            bwt,
            dict_file,
            password,
        } => {
            println!("라운드트립 자가 검증 테스트 시작: {:?}", input_paths);
            println!("테스트 알고리즘 모드: {:?}, 엔트로피 코딩: {:?}, 레벨: {}, 델타 필터: {}, BCJ 필터: {}, PNG 필터: {}, LPC 필터: {}, BWT 필터: {}",
                     mode, entropy, level, delta, bcj, png, lpc, bwt);
            if let Some(ref path) = dict_file {
                println!("사용할 사전 파일: {:?}", path);
            }
            if password.is_some() {
                println!("비밀번호 암호화 테스트가 켜졌습니다.");
            }

            // 원본 파일/디렉토리 로드 및 아카이빙
            let is_multi_or_dir = input_paths.len() > 1 || input_paths[0].is_dir();
            let original_bytes = if is_multi_or_dir {
                println!("여러 파일 또는 디렉토리가 감지되었습니다. MZAR 아카이브로 패킹을 먼저 진행합니다.");
                mzc::archive::archive_paths(&input_paths)
                    .with_context(|| format!("테스트 파일 아카이빙 실패: {:?}", input_paths))?
            } else {
                fs::read(&input_paths[0])
                    .with_context(|| format!("테스트 파일 '{:?}'을 읽을 수 없습니다.", input_paths[0]))?
            };

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
            let compressed_bytes = mzc::compress_bytes_v2_with_progress_dict_password(
                &original_bytes,
                mode,
                entropy,
                level,
                delta,
                bcj,
                png,
                lpc,
                bwt,
                dict_bytes.as_deref(),
                password.as_deref(),
                |_, _, _, _| {},
            );
            let total_compressed_size = compressed_bytes.len();

            // 2. 메모리상에서 즉각 해제 및 체크섬 검증
            let decompressed_bytes =
                mzc::decompress_bytes_v2_dict_password(&compressed_bytes, dict_bytes.as_deref(), password.as_deref())
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
        Commands::Train {
            input_files,
            output,
        } => {
            println!("사전 학습 시작 (총 {}개 파일)...", input_files.len());

            let mut all_bytes = Vec::new();
            for file_path in &input_files {
                println!("학습 대상 파일 로드: {:?}", file_path);
                let bytes = fs::read(file_path).with_context(|| {
                    format!("학습용 파일 '{:?}'을 읽을 수 없습니다.", file_path)
                })?;
                all_bytes.extend_from_slice(&bytes);
            }

            println!("해시 테이블 및 가중치 빈도 스캔을 통한 패턴 사전 추출 중...");
            let dict = mzc::rle::build_dictionary(&all_bytes);
            let dict_bytes = dict.to_bytes();

            fs::write(&output, &dict_bytes).with_context(|| {
                format!(
                    "학습된 사전 파일 '{:?}'을 저장하는 데 실패했습니다.",
                    output
                )
            })?;

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
        Commands::Inflate {
            input_file,
            output_file,
        } => {
            println!("Inflate 해제 구동: {:?} -> {:?}", input_file, output_file);

            let input_bytes = fs::read(&input_file)
                .with_context(|| format!("입력 파일 '{:?}'을 읽을 수 없습니다.", input_file))?;

            // GZIP 매직 2바이트 `1F 8B` 시작 여부에 따라 적절한 디코더 라이브러리를 바인딩합니다.
            let decompressed = if input_bytes.starts_with(&[0x1F, 0x8B]) {
                println!("GZIP 헤더 포맷 감지. RFC 1952 스펙에 맞춰 GZIP 체크섬 및 페이로드를 해제합니다.");
                mzc::deflate::gzip_decompress(&input_bytes).context("GZIP 압축 해제 오류 발생")?
            } else {
                println!("raw DEFLATE 비트스트림 감지. RFC 1951 스펙에 맞춰 동적 허프만 트리를 해제합니다.");
                mzc::deflate::inflate(&input_bytes).context("raw DEFLATE 압축 해제 오류 발생")?
            };

            fs::write(&output_file, &decompressed).with_context(|| {
                format!("복원 파일 '{:?}'을 저장하는 데 실패했습니다.", output_file)
            })?;

            println!(
                "압축 해제 및 복구 완료! 복원 파일 크기: {} bytes",
                decompressed.len()
            );
        }

        // --- MZC 설정 매트릭스 벤치마크 (Bench) 서브커맨드 실행 분기 ---
        Commands::Bench { input_file } => {
            println!("MZC Multi-Configuration Benchmarking Tool");
            println!("Target File: {:?}", input_file);

            let data = fs::read(&input_file)
                .with_context(|| format!("벤치마크 대상 파일 '{:?}'을 읽을 수 없습니다.", input_file))?;
            let orig_size = data.len();
            if orig_size == 0 {
                println!("Error: Empty files cannot be benchmarked.");
                return Ok(());
            }
            println!("Original Size: {} bytes", orig_size);
            println!("Running compression configurations matrix...\n");

            struct BenchResult {
                name: String,
                comp_size: usize,
                ratio: f64,
                comp_time_ms: f64,
                decomp_time_ms: f64,
                status: String,
            }

            // 비교할 압축 설정 조합 매트릭스 정의
            let matrix = vec![
                ("MZC1: Rle + None", mzc::CompressionMode::Rle, mzc::EntropyMode::None, false, false, false),
                ("MZC2: Hybrid + Huffman", mzc::CompressionMode::Hybrid, mzc::EntropyMode::Huffman, false, false, false),
                ("MZC4: Hybrid + Dynamic", mzc::CompressionMode::Hybrid, mzc::EntropyMode::Dynamic, false, false, false),
                ("MZC6: Hybrid + ANS", mzc::CompressionMode::Hybrid, mzc::EntropyMode::Ans, false, false, false),
                ("MZC7: Hybrid + CM", mzc::CompressionMode::Hybrid, mzc::EntropyMode::Cm, false, false, false),
                ("MZC3: LZ77 + Huffman", mzc::CompressionMode::Lz77, mzc::EntropyMode::Huffman, false, false, false),
                ("MZC4: LZ77 + Dynamic", mzc::CompressionMode::Lz77, mzc::EntropyMode::Dynamic, false, false, false),
                ("MZC6: LZ77 + ANS", mzc::CompressionMode::Lz77, mzc::EntropyMode::Ans, false, false, false),
                ("MZC7: LZ77 + CM", mzc::CompressionMode::Lz77, mzc::EntropyMode::Cm, false, false, false),
                ("MZC7: LZ77 + CM + BWT", mzc::CompressionMode::Lz77, mzc::EntropyMode::Cm, false, false, true),
                ("MZC5: LZ77 + Dynamic + Delta + BCJ", mzc::CompressionMode::Lz77, mzc::EntropyMode::Dynamic, true, true, false),
            ];

            let mut results = Vec::new();

            for (name, mode, entropy, delta, bcj, bwt) in matrix {
                print!("Running {}... ", name);
                std::io::Write::flush(&mut std::io::stdout())?;

                let start_comp = std::time::Instant::now();
                let compressed = mzc::compress_bytes_v2_with_progress_dict_password(
                    &data,
                    mode,
                    entropy,
                    6,
                    delta,
                    bcj,
                    false, // png
                    false, // lpc
                    bwt,
                    None,
                    None,
                    |_, _, _, _| {},
                );
                let comp_time = start_comp.elapsed().as_secs_f64() * 1000.0;
                let comp_size = compressed.len();
                let ratio = (comp_size as f64 / orig_size as f64) * 100.0;

                let start_decomp = std::time::Instant::now();
                let decompressed = mzc::decompress_bytes_v2_dict_password(&compressed, None, None);
                let decomp_time = start_decomp.elapsed().as_secs_f64() * 1000.0;

                let status = match decompressed {
                    Ok(ref restored) if restored == &data => "OK".to_string(),
                    Ok(_) => "MISMATCH".to_string(),
                    Err(e) => format!("FAIL ({})", e),
                };

                println!("Done! Ratio: {:.2}%, C_Time: {:.1}ms", ratio, comp_time);

                results.push(BenchResult {
                    name: name.to_string(),
                    comp_size,
                    ratio,
                    comp_time_ms: comp_time,
                    decomp_time_ms: decomp_time,
                    status,
                });
            }

            // 압축비 기준 오름차순(우수한 압축률 우선) 정렬
            results.sort_by(|a, b| a.ratio.partial_cmp(&b.ratio).unwrap());

            // 벤치마크 결과 비교 표 출력
            println!("\n+--------------------------------------+---------------+------------+-----------------+-----------------+--------+");
            println!("| Configuration                        | Comp Size (B) | Ratio (%)  | Comp Time (ms)  | Dec Time (ms)   | Status |");
            println!("+--------------------------------------+---------------+------------+-----------------+-----------------+--------+");
            for res in results {
                println!(
                    "| {:<36} | {:>13} | {:>9.2}% | {:>14.1}  | {:>14.1}  | {:<6} |",
                    res.name, res.comp_size, res.ratio, res.comp_time_ms, res.decomp_time_ms, res.status
                );
            }
            println!("+--------------------------------------+---------------+------------+-----------------+-----------------+--------+");
        }

        // --- 데스크톱 그래픽 GUI 애플리케이션 실행 분기 ---
        Commands::Gui => {
            println!("MZC 데스크톱 GUI 애플리케이션을 시작합니다...");
            mzc::gui::run_gui_app().map_err(|e| anyhow::anyhow!("GUI 앱 구동 실패: {}", e))?;
        }

        // --- 윈도우 마우스 우클릭 메뉴 등록 실행 분기 ---
        Commands::RegisterContextMenu => {
            mzc::register_context_menu()?;
        }

        // --- 윈도우 마우스 우클릭 메뉴 해제 실행 분기 ---
        Commands::UnregisterContextMenu => {
            mzc::unregister_context_menu()?;
        }
    }

    Ok(())
}
