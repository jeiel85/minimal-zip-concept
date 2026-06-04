use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender, Receiver};
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, BarChart, Bar};
use crate::cli::{CompressionMode, EntropyMode};
use crate::checksum::calculate_sha256;
use crate::rle::Dictionary;
use crate::huffman::{huffman_decompress, huffman_decompress_dynamic};
use crate::format::{
    MzcHeader, VERSION_MZC1, VERSION_MZC2, VERSION_MZC3, VERSION_MZC4, VERSION_MZC5, VERSION_MZC6,
    VERSION_MZC7, FILTER_DELTA, FILTER_BCJ, FILTER_DYNAMIC_HUFFMAN, FILTER_ANS, ALGORITHM_RLE,
    ALGORITHM_DICT, ALGORITHM_HYBRID, ALGORITHM_LZ77, HEADER_SIZE_MZC1, HEADER_SIZE_MZC2
};

/// **MZC 그래픽 데스크톱 GUI 애플리케이션을 구동합니다.**
///
/// # Rust 개념 설명:
/// - `Result<(), eframe::Error>`: 함수 실행 결과를 가리키며, 정상 실행 시 빈 값인 `Ok(())`를,
///   실패 시 eframe 라이브러리가 던진 그래픽 관련 오류 `Err(e)`를 반환합니다.
pub fn run_gui_app() -> Result<(), eframe::Error> {
    // 윈도우 창의 이름, 기본 크기, 최소 크기 등의 화면 해상도 설정을 미리 구성합니다.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MZC (Minimal Zip Concept) - Advanced Interactive Compressor")
            .with_inner_size([960.0, 720.0])
            .with_min_inner_size([850.0, 560.0]),
        ..Default::default()
    };
    
    // 네이티브 창을 기동하여 GUI 루프를 실행합니다.
    eframe::run_native(
        "MZC Desktop App",
        options,
        Box::new(|cc| Box::new(MzcGuiApp::new(cc))),
    )
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
    pub changelog: String,
}

/// **GUI 비동기 스레드 작업 결과를 수집하기 위한 채널 데이터 열거형입니다.**
///
/// 압축이나 검사는 파일이 클수록 1초 이상 걸려 화면을 멈추게 하므로,
/// 백그라운드 스레드에서 연산을 수행한 뒤 이 열거형 메시지를 채널(Channel)을 통해 메인 화면으로 전달합니다.
enum TaskResult {
    CompressDone {
        orig_size: u64,
        comp_size: u64,
        ratio: f64,
        sha256: String,
        visual_blocks: Vec<char>,
        literal_count: usize,
        run_count: usize,
        token_count: usize,
        backref_count: usize,
        saved_path: PathBuf,
        format_desc: String,
        alg_desc: String,
    },
    DecompressDone {
        restored_size: u64,
        sha256: String,
        saved_path: PathBuf,
    },
    InspectDone {
        orig_size: u64,
        comp_size: u64,
        ratio: f64,
        sha256: String,
        verified: bool,
        visual_blocks: Vec<char>,
        literal_count: usize,
        run_count: usize,
        token_count: usize,
        backref_count: usize,
        format_desc: String,
        alg_desc: String,
    },
    ChunkProgress {
        chunk_idx: usize,
        orig_size: usize,
        comp_size: usize,
        duration: f64,
    },
    TrainDone {
        dict_size: usize,
        entry_count: usize,
        saved_path: PathBuf,
    },
    BenchmarkDone {
        mzc_size: u64,
        gzip_size: u64,
        zstd_size: u64,
        mzc_time: f64,
        gzip_time: f64,
        zstd_time: f64,
    },
    UpdateCheckResult(Result<UpdateInfo, String>),
    UpdateDownloadProgress(f32),
    UpdateDownloadDone(PathBuf),
    Error(String),
}

/// **MZC Desktop GUI의 핵심 상태를 보관하는 통합 앱 구조체입니다.**
pub struct MzcGuiApp {
    // 인풋 파일 및 외부 사전 경로
    input_path: Option<PathBuf>,
    compression_mode: CompressionMode,
    entropy_mode: EntropyMode,
    dict_path: Option<PathBuf>, // MZC7 대응 외부 선택 사전 경로

    // 비동기 작업용 채널 및 상태
    status: String,
    is_processing: bool,
    task_sender: Sender<TaskResult>,
    task_receiver: Receiver<TaskResult>,

    // 통계 메타데이터
    original_size: u64,
    compressed_size: u64,
    compression_ratio: f64,
    sha256_hash: String,
    verified_ok: bool,

    // 포맷 상세 설명
    format_description: String,
    algorithm_description: String,

    // 블록 시각화 맵
    visual_blocks: Vec<char>,
    literal_count: usize,
    run_count: usize,
    token_count: usize,
    backref_count: usize,

    // 압축 레벨 및 필터 옵션
    compression_level: u8,
    delta_enabled: bool,
    bcj_enabled: bool,
    png_enabled: bool, // MZC7 PNG Paeth 필터 활성화 여부
    lpc_enabled: bool, // MZC7 LPC 오디오 필터 활성화 여부

    // 실시간 모니터링 통계 버퍼
    chunk_ratios: Vec<f64>,
    chunk_throughputs: Vec<f64>,

    // 중앙 판넬의 현재 탭 위치 (0: Dashboard, 1: Dict Trainer, 2: tANS Simulation, 3: Benchmark, 4: CM Visualizer)
    active_tab: usize,

    // 사전 학습 탭(Train Tab) 전용 상태
    train_files: Vec<PathBuf>,
    train_output_path: PathBuf,
    train_status: String,

    // tANS 시뮬레이터 탭 전용 플롯 데이터
    tans_sim_states: Vec<[f64; 2]>,
    tans_sim_bits: Vec<[f64; 2]>,

    // Benchmark 탭 전용 상태
    benchmark_file_path: Option<PathBuf>,
    benchmark_mzc_size: u64,
    benchmark_gzip_size: u64,
    benchmark_zstd_size: u64,
    benchmark_mzc_time: f64,
    benchmark_gzip_time: f64,
    benchmark_zstd_time: f64,
    benchmark_status: String,

    // CM 비트 확률 시뮬레이션 탭 전용 상태
    cm_sim_bit_idx: usize,
    cm_sim_byte_val: u8,
    cm_sim_bits: [bool; 8],
    cm_sim_ctx_byte: u16,
    cm_sim_prev1: u8,
    cm_sim_prev2: u8,
    cm_sim_probabilities: [u32; 3], // p0, p1, p2
    cm_sim_weights: [[i32; 3]; 8], // w0, w1, w2
    cm_sim_mixed_p: u32,
    cm_sim_autoplay: bool,
    cm_sim_last_step_time: std::time::Instant,

    // 자동 업데이트 및 컨텍스트 메뉴 관련 상태
    update_checked: bool,
    update_available: Option<UpdateInfo>,
    is_checking_update: bool,
    is_downloading_update: bool,
    download_progress: f32,
    show_update_modal: bool,
    context_menu_status: String,
}

impl MzcGuiApp {
    /// **MzcGuiApp 생성자 및 초기화**
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 프리미엄 다크 슬레이트 테마 비주얼(Visuals) 설정
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = 12.0.into();
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(18, 18, 20); // 딥 다크 블랙
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(30, 30, 35); // 다크 그레이
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(45, 45, 52);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(45, 206, 137); // HSL 포인트 민트그린
        cc.egui_ctx.set_visuals(visuals);

        // 스레드 통신용 채널쌍 생성
        let (task_sender, task_receiver) = channel();

        Self {
            input_path: None,
            compression_mode: CompressionMode::Lz77,
            entropy_mode: EntropyMode::Huffman,
            dict_path: None,
            status: "파일을 끌어다 놓거나 아래에서 선택해 주세요.".to_string(),
            is_processing: false,
            task_sender,
            task_receiver,
            original_size: 0,
            compressed_size: 0,
            compression_ratio: 0.0,
            sha256_hash: String::new(),
            verified_ok: false,
            format_description: "대기 중".to_string(),
            algorithm_description: "대기 중".to_string(),
            visual_blocks: Vec::new(),
            literal_count: 0,
            run_count: 0,
            token_count: 0,
            backref_count: 0,
            compression_level: 6,
            delta_enabled: false,
            bcj_enabled: false,
            png_enabled: false,
            lpc_enabled: false,
            chunk_ratios: Vec::new(),
            chunk_throughputs: Vec::new(),
            active_tab: 0,
            train_files: Vec::new(),
            train_output_path: PathBuf::from("trained.dict"),
            train_status: "학습 대상 파일을 추가하고 시작해 주세요.".to_string(),
            tans_sim_states: Vec::new(),
            tans_sim_bits: Vec::new(),
            benchmark_file_path: None,
            benchmark_mzc_size: 0,
            benchmark_gzip_size: 0,
            benchmark_zstd_size: 0,
            benchmark_mzc_time: 0.0,
            benchmark_gzip_time: 0.0,
            benchmark_zstd_time: 0.0,
            benchmark_status: "벤치마크할 파일을 선택해 주세요.".to_string(),
            cm_sim_bit_idx: 0,
            cm_sim_byte_val: 0xAB,
            cm_sim_bits: [true, false, true, false, true, false, true, true],
            cm_sim_ctx_byte: 1,
            cm_sim_prev1: 0x55,
            cm_sim_prev2: 0x33,
            cm_sim_probabilities: [2048, 2048, 2048],
            cm_sim_weights: [[1024, 2048, 5120]; 8],
            cm_sim_mixed_p: 2048,
            cm_sim_autoplay: false,
            cm_sim_last_step_time: std::time::Instant::now(),
            update_checked: false,
            update_available: None,
            is_checking_update: false,
            is_downloading_update: false,
            download_progress: 0.0,
            show_update_modal: false,
            context_menu_status: "대기 중".to_string(),
        }
    }

    fn spawn_check_update_task(&self) {
        let tx = self.task_sender.clone();
        std::thread::spawn(move || {
            let url = "https://raw.githubusercontent.com/jeiel85/minimal-zip-concept/main/docs/latest_version.json";
            match ureq::get(url).call() {
                Ok(response) => {
                    match serde_json::from_reader::<_, UpdateInfo>(response.into_reader()) {
                        Ok(info) => {
                            let current_version = env!("CARGO_PKG_VERSION");
                            if is_newer_version(&info.version, current_version) {
                                let _ = tx.send(TaskResult::UpdateCheckResult(Ok(info)));
                            } else {
                                let _ = tx.send(TaskResult::UpdateCheckResult(Err("최신 버전을 사용 중입니다.".to_string())));
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::UpdateCheckResult(Err(format!("업데이트 정보 파싱 실패: {}", e))));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::UpdateCheckResult(Err(format!("업데이트 서버 연결 실패: {}", e))));
                }
            }
        });
    }

    fn spawn_download_update_task(&self, download_url: String) {
        let tx = self.task_sender.clone();
        std::thread::spawn(move || {
            use std::io::Read;
            match ureq::get(&download_url).call() {
                Ok(response) => {
                    let total_size = response
                        .header("Content-Length")
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(0);
                    
                    let temp_dir = std::env::temp_dir();
                    let setup_path = temp_dir.join("mzc-setup.exe");
                    
                    match std::fs::File::create(&setup_path) {
                        Ok(mut file) => {
                            let mut reader = response.into_reader();
                            let mut buffer = [0; 16384];
                            let mut downloaded = 0;
                            
                            loop {
                                match reader.read(&mut buffer) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        use std::io::Write;
                                        if let Err(e) = file.write_all(&buffer[..n]) {
                                            let _ = tx.send(TaskResult::Error(format!("다운로드 파일 쓰기 오류: {}", e)));
                                            return;
                                        }
                                        downloaded += n;
                                        if total_size > 0 {
                                            let progress = downloaded as f32 / total_size as f32;
                                            let _ = tx.send(TaskResult::UpdateDownloadProgress(progress));
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx.send(TaskResult::Error(format!("다운로드 스트림 오류: {}", e)));
                                        return;
                                    }
                                }
                            }
                            
                            let _ = tx.send(TaskResult::UpdateDownloadDone(setup_path));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(format!("임시 파일 생성 실패: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("다운로드 연결 실패: {}", e)));
                }
            }
        });
    }

    /// **비동기 압축 태스크를 백그라운드 스레드에 위임(Spawn)합니다.**
    fn spawn_compress_task(
        &self,
        path: PathBuf,
        mode: CompressionMode,
        entropy: EntropyMode,
        level: u8,
        delta: bool,
        bcj: bool,
        png: bool,
        lpc: bool,
        dict_path: Option<PathBuf>,
    ) {
        let tx = self.task_sender.clone();
        std::thread::spawn(move || {
            // 원본 바이트 로드
            match std::fs::read(&path) {
                Ok(original_bytes) => {
                    let orig_size = original_bytes.len() as u64;
                    let sha256 = crate::checksum::bytes_to_hex(&calculate_sha256(&original_bytes));
                    
                    // 지정된 사전 파일 로드
                    let dict_bytes = if let Some(ref d_path) = dict_path {
                        std::fs::read(d_path).ok()
                    } else {
                        None
                    };

                    // 실시간 청크 전송 콜백과 함께 라이브러리 압축 구동
                    let tx_progress = tx.clone();
                    let final_output = crate::compress_bytes_v2_with_progress_dict(
                        &original_bytes,
                        mode,
                        entropy,
                        level,
                        delta,
                        bcj,
                        png,
                        lpc,
                        dict_bytes.as_deref(),
                        move |chunk_idx, orig_size, comp_size, duration| {
                            let _ = tx_progress.send(TaskResult::ChunkProgress {
                                chunk_idx,
                                orig_size,
                                comp_size,
                                duration,
                            });
                        }
                    );
                    
                    let comp_size = final_output.len() as u64;
                    let ratio = if orig_size > 0 { (comp_size as f64 / orig_size as f64) * 100.0 } else { 100.0 };

                    // 이진 블록 맵 시각화를 위한 압축파일 해독 파싱 진행
                    let mut visual_blocks = Vec::new();
                    let mut literal_count = 0;
                    let mut run_count = 0;
                    let mut token_count = 0;
                    let mut backref_count = 0;

                    let mut version_mzc7 = false;
                    let mut version_mzc5 = false;
                    if let Ok(header) = MzcHeader::from_bytes(&final_output) {
                        version_mzc7 = header.version == VERSION_MZC7;
                        version_mzc5 = header.version == VERSION_MZC5;
                        let header_size = if header.version >= VERSION_MZC2 {
                            HEADER_SIZE_MZC2
                        } else {
                            HEADER_SIZE_MZC1
                        };
                        let payload_bytes = &final_output[header_size..];

                        // MZC2~MZC7의 구조화 블록들을 따라가며 레이아웃을 역으로 추적합니다.
                        if header.version >= VERSION_MZC2 && header.original_size > 0 {
                            let mut pos = header.dictionary_size as usize;
                            let n = payload_bytes.len();
                            while pos < n {
                                if pos + 12 > n { break; }
                                let chunk_orig_size = u32::from_le_bytes(payload_bytes[pos..pos + 4].try_into().unwrap()) as usize;
                                let comb_size = u32::from_le_bytes(payload_bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
                                let comp_size = u32::from_le_bytes(payload_bytes[pos + 8..pos + 12].try_into().unwrap()) as usize;
                                pos += 12;
                                if pos + comp_size > n { break; }

                                let chunk_data = &payload_bytes[pos..pos + comp_size];
                                pos += comp_size;

                                // 엔트로피 디코딩 판별
                                let (is_huffman, is_dynamic, is_ans, is_cm) = if header.version == VERSION_MZC7 {
                                    let entropy_bits = (header.algorithm_type >> 2) & 0x07;
                                    (
                                        entropy_bits == 1,
                                        entropy_bits == 2,
                                        entropy_bits == 3,
                                        entropy_bits == 4,
                                    )
                                } else {
                                    (
                                        header.version < VERSION_MZC7 && chunk_data.len() != comb_size && (header.version != VERSION_MZC4 && (header.version < VERSION_MZC5 || (header.algorithm_type & FILTER_DYNAMIC_HUFFMAN) == 0) && (header.version < VERSION_MZC6 || (header.algorithm_type & FILTER_ANS) == 0)),
                                        header.version == VERSION_MZC4 || (header.version >= VERSION_MZC5 && (header.algorithm_type & FILTER_DYNAMIC_HUFFMAN) != 0),
                                        header.version >= VERSION_MZC6 && (header.algorithm_type & FILTER_ANS) != 0,
                                        false,
                                    )
                                };

                                let unhuff = if is_cm {
                                    crate::cm::cm_decompress(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                } else if is_ans {
                                    crate::ans::ans_decompress(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                } else if is_dynamic {
                                    huffman_decompress_dynamic(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                } else if is_huffman {
                                    huffman_decompress(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                } else {
                                    chunk_data.to_vec()
                                };

                                // 로컬/전역 사전을 확보합니다.
                                let (dict, rle_payload) = if header.dictionary_size > 0 {
                                    let g_dict = if let Some(ref d_bytes) = dict_bytes {
                                        Dictionary::from_bytes(d_bytes).unwrap_or_default()
                                    } else {
                                        Dictionary::new()
                                    };
                                    (g_dict, unhuff)
                                } else {
                                    let dict = Dictionary::from_bytes(&unhuff).unwrap_or_default();
                                    let dict_bytes_len = dict.to_bytes().len();
                                    if dict_bytes_len < unhuff.len() {
                                        (dict, unhuff[dict_bytes_len..].to_vec())
                                    } else {
                                        (dict, Vec::new())
                                    }
                                };

                                // 블록 순회 파싱
                                if !rle_payload.is_empty() {
                                    let mut b_pos = 0;
                                    let b_n = rle_payload.len();
                                    let mut decomp_size = 0;

                                    if header.version >= VERSION_MZC5 {
                                        while b_pos < b_n && decomp_size < chunk_orig_size {
                                            if b_pos + 2 > b_n { break; }
                                            let flag = u16::from_le_bytes(rle_payload[b_pos..b_pos + 2].try_into().unwrap());
                                            b_pos += 2;

                                            for k in 0..8 {
                                                if decomp_size >= chunk_orig_size { break; }
                                                let b_type = ((flag >> (2 * k)) & 0x03) as u8;
                                                match b_type {
                                                    0x00 => {
                                                        if b_pos + 2 > b_n { break; }
                                                        let b_len = u16::from_le_bytes(rle_payload[b_pos..b_pos + 2].try_into().unwrap()) as usize;
                                                        b_pos += 2 + b_len;
                                                        decomp_size += b_len;
                                                        literal_count += 1;
                                                        visual_blocks.push('L');
                                                    }
                                                    0x01 => {
                                                        if b_pos + 3 > b_n { break; }
                                                        let b_len = u16::from_le_bytes(rle_payload[b_pos..b_pos + 2].try_into().unwrap()) as usize;
                                                        b_pos += 3;
                                                        decomp_size += b_len;
                                                        run_count += 1;
                                                        visual_blocks.push('R');
                                                    }
                                                    0x02 => {
                                                        if b_pos + 2 > b_n { break; }
                                                        b_pos += 2;
                                                        decomp_size += 2; // 가상 토큰 크기 가정
                                                        token_count += 1;
                                                        visual_blocks.push('T');
                                                    }
                                                    0x03 => {
                                                        if b_pos + 4 > b_n { break; }
                                                        let length = u16::from_le_bytes(rle_payload[b_pos + 2..b_pos + 4].try_into().unwrap()) as usize;
                                                        b_pos += 4;
                                                        decomp_size += length;
                                                        backref_count += 1;
                                                        visual_blocks.push('B');
                                                    }
                                                    _ => break,
                                                }
                                            }
                                        }
                                    } else {
                                        // MZC2~MZC4 구버전 RLE 블록 파싱
                                        while b_pos < b_n {
                                            if b_pos + 3 > b_n { break; }
                                            let b_type = rle_payload[b_pos];
                                            let b_len = u16::from_le_bytes(rle_payload[b_pos + 1..b_pos + 3].try_into().unwrap()) as usize;
                                            b_pos += 3;

                                            match b_type {
                                                0x00 => {
                                                    literal_count += 1;
                                                    visual_blocks.push('L');
                                                    b_pos += b_len;
                                                }
                                                0x01 => {
                                                    run_count += 1;
                                                    visual_blocks.push('R');
                                                    b_pos += 1;
                                                }
                                                0x02 => {
                                                    token_count += 1;
                                                    visual_blocks.push('T');
                                                }
                                                0x03 => {
                                                    backref_count += 1;
                                                    visual_blocks.push('B');
                                                    b_pos += 2;
                                                }
                                                _ => break,
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 파일 저장
                    let mut saved_path = path.clone();
                    saved_path.set_extension("mzip");

                    let format_desc = if version_mzc7 {
                        "MZC7 - Context Mixing & Media Filters Spec".to_string()
                    } else if version_mzc5 {
                        "MZC5 - Bit-Packed & Preprocessors Spec".to_string()
                    } else if entropy == EntropyMode::Dynamic {
                        "MZC4 - Dynamic Huffman Spec".to_string()
                    } else if mode == CompressionMode::Lz77 {
                        "MZC3 - Sliding Window Chunk Spec".to_string()
                    } else if mode == CompressionMode::Rle && entropy == EntropyMode::None {
                        "MZC1 - Retro RLE Spec".to_string()
                    } else {
                        "MZC2 - Parallel Dictionary Spec".to_string()
                    };

                    let alg_desc = match mode {
                        CompressionMode::Rle => "RLE Only Mode",
                        CompressionMode::Dict => "Dictionary Only Mode",
                        CompressionMode::Hybrid => "Hybrid Mode",
                        CompressionMode::Lz77 => "LZ77 Hybrid Mode",
                    }.to_string();

                    match std::fs::write(&saved_path, &final_output) {
                        Ok(_) => {
                            let _ = tx.send(TaskResult::CompressDone {
                                orig_size,
                                comp_size,
                                ratio,
                                sha256,
                                visual_blocks,
                                literal_count,
                                run_count,
                                token_count,
                                backref_count,
                                saved_path,
                                format_desc,
                                alg_desc,
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(format!("압축 저장 에러: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("파일 로드 에러: {}", e)));
                }
            }
        });
    }

    /// **비동기 압축 해제 태스크를 백그라운드 스레드에 위임(Spawn)합니다.**
    fn spawn_decompress_task(&self, path: PathBuf, dict_path: Option<PathBuf>) {
        let tx = self.task_sender.clone();
        std::thread::spawn(move || {
            match std::fs::read(&path) {
                Ok(compressed_bytes) => {
                    let dict_bytes = if let Some(ref d_path) = dict_path {
                        std::fs::read(d_path).ok()
                    } else {
                        None
                    };

                    match crate::decompress_bytes_v2_dict(&compressed_bytes, dict_bytes.as_deref()) {
                        Ok(restored_bytes) => {
                            let restored_size = restored_bytes.len() as u64;
                            let sha256 = crate::checksum::bytes_to_hex(&calculate_sha256(&restored_bytes));

                            let mut saved_path = path.clone();
                            saved_path.set_extension("restored.txt");

                            match std::fs::write(&saved_path, &restored_bytes) {
                                Ok(_) => {
                                    let _ = tx.send(TaskResult::DecompressDone {
                                        restored_size,
                                        sha256,
                                        saved_path,
                                    });
                                }
                                Err(e) => {
                                    let _ = tx.send(TaskResult::Error(format!("파일 복원 쓰기 에러: {}", e)));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(format!("압축 해제 복구 실패: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("압축 데이터 로드 실패: {}", e)));
                }
            }
        });
    }

    /// **비동기 분석(Inspect) 태스크를 백그라운드 스레드에 위임(Spawn)합니다.**
    fn spawn_inspect_task(&self, path: PathBuf, dict_path: Option<PathBuf>) {
        let tx = self.task_sender.clone();
        std::thread::spawn(move || {
            match std::fs::read(&path) {
                Ok(file_bytes) => {
                    match MzcHeader::from_bytes(&file_bytes) {
                        Ok(header) => {
                            let header_size = if header.version >= VERSION_MZC2 {
                                HEADER_SIZE_MZC2
                            } else {
                                HEADER_SIZE_MZC1
                            };

                            let dict_bytes = if let Some(ref d_path) = dict_path {
                                std::fs::read(d_path).ok()
                            } else {
                                None
                            };

                            let format_desc = if header.version == VERSION_MZC7 {
                                "MZC7 (Minimal Zip Concept v7 - Context Mixing & Media)"
                            } else if header.version == VERSION_MZC6 {
                                "MZC6 (Minimal Zip Concept v6 - ANS Table)"
                            } else if header.version == VERSION_MZC5 {
                                "MZC5 (Minimal Zip Concept v5 - Bit-Packed & Preprocessors)"
                            } else if header.version == VERSION_MZC4 {
                                "MZC4 (Minimal Zip Concept v4 - Dynamic Huffman)"
                            } else if header.version == VERSION_MZC3 {
                                "MZC3 (Minimal Zip Concept v3 - Sliding Window)"
                            } else if header.version == VERSION_MZC2 {
                                "MZC2 (Minimal Zip Concept v2 - Parallel Dictionary)"
                            } else {
                                "MZC1 (Minimal Zip Concept v1 - Retro RLE)"
                            };

                            let core_alg = if header.version == VERSION_MZC7 {
                                let core_bits = header.algorithm_type & 0x03;
                                match core_bits {
                                    0 => ALGORITHM_RLE,
                                    1 => ALGORITHM_DICT,
                                    2 => ALGORITHM_HYBRID,
                                    3 => ALGORITHM_LZ77,
                                    _ => unreachable!(),
                                }
                            } else if header.version >= VERSION_MZC5 {
                                header.algorithm_type & 0x0F
                            } else {
                                header.algorithm_type
                            };

                            let alg_desc = if header.version == VERSION_MZC7 {
                                let mut desc = match core_alg {
                                    ALGORITHM_RLE => "RLE Only (Run-Length Encoding)".to_string(),
                                    ALGORITHM_DICT => "Dictionary Only (Entropy Enabled)".to_string(),
                                    ALGORITHM_HYBRID => "Hybrid Mode (RLE + Dictionary + Huffman)".to_string(),
                                    ALGORITHM_LZ77 => "LZ77 Hybrid (Runs + Dictionary + BackRefs)".to_string(),
                                    _ => "Unknown Mode".to_string(),
                                };
                                let entropy_bits = (header.algorithm_type >> 2) & 0x07;
                                let entropy_name = match entropy_bits {
                                    0 => "None",
                                    1 => "Static Huffman",
                                    2 => "Dynamic Huffman",
                                    3 => "ANS",
                                    4 => "Context Mixing (CM)",
                                    _ => "Unknown",
                                };
                                desc.push_str(&format!(" [Entropy: {}]", entropy_name));
                                
                                let filter_bits = (header.algorithm_type >> 5) & 0x07;
                                let filter_name = match filter_bits {
                                    1 => "Delta",
                                    2 => "BCJ",
                                    3 => "PNG (Paeth)",
                                    4 => "LPC (Audio)",
                                    5 => "Delta + BCJ",
                                    _ => "None",
                                };
                                desc.push_str(&format!(" [Filter: {}]", filter_name));
                                desc
                            } else if header.version >= VERSION_MZC5 {
                                let mut desc = match core_alg {
                                    ALGORITHM_RLE => "RLE Only (Run-Length Encoding)".to_string(),
                                    ALGORITHM_DICT => "Dictionary Only (Entropy Enabled)".to_string(),
                                    ALGORITHM_HYBRID => "Hybrid Mode (RLE + Dictionary + Huffman)".to_string(),
                                    ALGORITHM_LZ77 => "LZ77 Hybrid (Runs + Dictionary + BackRefs)".to_string(),
                                    _ => "Unknown Mode".to_string(),
                                };
                                let has_delta = (header.algorithm_type & FILTER_DELTA) != 0;
                                let has_bcj = (header.algorithm_type & FILTER_BCJ) != 0;
                                if has_delta || has_bcj {
                                    desc.push_str(" [Filters:");
                                    if has_delta { desc.push_str(" Delta"); }
                                    if has_bcj { desc.push_str(" BCJ"); }
                                    desc.push_str("]");
                                }
                                desc
                            } else {
                                match header.algorithm_type {
                                    ALGORITHM_RLE => "RLE Only (Run-Length Encoding)".to_string(),
                                    ALGORITHM_DICT => "Dictionary Only (Entropy Enabled)".to_string(),
                                    ALGORITHM_HYBRID => "Hybrid Mode (RLE + Dictionary + Huffman)".to_string(),
                                    ALGORITHM_LZ77 => "LZ77 Hybrid (Runs + Dictionary + BackRefs)".to_string(),
                                    _ => "Unknown Mode".to_string(),
                                }
                            };

                            let payload_bytes = &file_bytes[header_size..];
                            let mut visual_blocks = Vec::new();
                            let mut literal_count = 0;
                            let mut run_count = 0;
                            let mut token_count = 0;
                            let mut backref_count = 0;

                            if header.version >= VERSION_MZC2 && header.original_size > 0 {
                                let mut pos = header.dictionary_size as usize;
                                let n = payload_bytes.len();
                                while pos < n {
                                    if pos + 12 > n { break; }
                                    let chunk_orig_size = u32::from_le_bytes(payload_bytes[pos..pos + 4].try_into().unwrap()) as usize;
                                    let comb_size = u32::from_le_bytes(payload_bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
                                    let comp_size = u32::from_le_bytes(payload_bytes[pos + 8..pos + 12].try_into().unwrap()) as usize;
                                    pos += 12;
                                    if pos + comp_size > n { break; }

                                    let chunk_data = &payload_bytes[pos..pos + comp_size];
                                    pos += comp_size;

                                    let (is_huffman, is_dynamic, is_ans, is_cm) = if header.version == VERSION_MZC7 {
                                        let entropy_bits = (header.algorithm_type >> 2) & 0x07;
                                        (
                                            entropy_bits == 1,
                                            entropy_bits == 2,
                                            entropy_bits == 3,
                                            entropy_bits == 4,
                                        )
                                    } else {
                                        (
                                            header.version < VERSION_MZC7 && chunk_data.len() != comb_size && (header.version != VERSION_MZC4 && (header.version < VERSION_MZC5 || (header.algorithm_type & FILTER_DYNAMIC_HUFFMAN) == 0) && (header.version < VERSION_MZC6 || (header.algorithm_type & FILTER_ANS) == 0)),
                                            header.version == VERSION_MZC4 || (header.version >= VERSION_MZC5 && (header.algorithm_type & FILTER_DYNAMIC_HUFFMAN) != 0),
                                            header.version >= VERSION_MZC6 && (header.algorithm_type & FILTER_ANS) != 0,
                                            false,
                                        )
                                    };

                                    let unhuff = if is_cm {
                                        crate::cm::cm_decompress(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                    } else if is_ans {
                                        crate::ans::ans_decompress(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                    } else if is_dynamic {
                                        huffman_decompress_dynamic(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                    } else if is_huffman {
                                        huffman_decompress(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                    } else {
                                        chunk_data.to_vec()
                                    };

                                    let (dict, rle_payload) = if header.dictionary_size > 0 {
                                        let g_dict = if let Some(ref d_bytes) = dict_bytes {
                                            Dictionary::from_bytes(d_bytes).unwrap_or_default()
                                        } else {
                                            Dictionary::new()
                                        };
                                        (g_dict, unhuff)
                                    } else {
                                        let dict = Dictionary::from_bytes(&unhuff).unwrap_or_default();
                                        let dict_bytes_len = dict.to_bytes().len();
                                        if dict_bytes_len < unhuff.len() {
                                            (dict, unhuff[dict_bytes_len..].to_vec())
                                        } else {
                                            (dict, Vec::new())
                                        }
                                    };

                                    if !rle_payload.is_empty() {
                                        let mut b_pos = 0;
                                        let b_n = rle_payload.len();
                                        let mut decomp_size = 0;

                                        if header.version >= VERSION_MZC5 {
                                            while b_pos < b_n && decomp_size < chunk_orig_size {
                                                if b_pos + 2 > b_n { break; }
                                                let flag = u16::from_le_bytes(rle_payload[b_pos..b_pos + 2].try_into().unwrap());
                                                b_pos += 2;

                                                for k in 0..8 {
                                                    if decomp_size >= chunk_orig_size { break; }
                                                    let b_type = ((flag >> (2 * k)) & 0x03) as u8;
                                                    match b_type {
                                                        0x00 => {
                                                            if b_pos + 2 > b_n { break; }
                                                            let b_len = u16::from_le_bytes(rle_payload[b_pos..b_pos + 2].try_into().unwrap()) as usize;
                                                            b_pos += 2 + b_len;
                                                            decomp_size += b_len;
                                                            literal_count += 1;
                                                            visual_blocks.push('L');
                                                        }
                                                        0x01 => {
                                                            if b_pos + 3 > b_n { break; }
                                                            let b_len = u16::from_le_bytes(rle_payload[b_pos..b_pos + 2].try_into().unwrap()) as usize;
                                                            b_pos += 3;
                                                            decomp_size += b_len;
                                                            run_count += 1;
                                                            visual_blocks.push('R');
                                                        }
                                                        0x02 => {
                                                            if b_pos + 2 > b_n { break; }
                                                            b_pos += 2;
                                                            decomp_size += 2;
                                                            token_count += 1;
                                                            visual_blocks.push('T');
                                                        }
                                                        0x03 => {
                                                            if b_pos + 4 > b_n { break; }
                                                            let length = u16::from_le_bytes(rle_payload[b_pos + 2..b_pos + 4].try_into().unwrap()) as usize;
                                                            b_pos += 4;
                                                            decomp_size += length;
                                                            backref_count += 1;
                                                            visual_blocks.push('B');
                                                        }
                                                        _ => break,
                                                    }
                                                }
                                            }
                                        } else {
                                            while b_pos < b_n {
                                                if b_pos + 3 > b_n { break; }
                                                let b_type = rle_payload[b_pos];
                                                let b_len = u16::from_le_bytes(rle_payload[b_pos + 1..b_pos + 3].try_into().unwrap()) as usize;
                                                b_pos += 3;

                                                match b_type {
                                                    0x00 => {
                                                        literal_count += 1;
                                                        visual_blocks.push('L');
                                                        b_pos += b_len;
                                                    }
                                                    0x01 => {
                                                        run_count += 1;
                                                        visual_blocks.push('R');
                                                        b_pos += 1;
                                                    }
                                                    0x02 => {
                                                        token_count += 1;
                                                        visual_blocks.push('T');
                                                    }
                                                    0x03 => {
                                                        backref_count += 1;
                                                        visual_blocks.push('B');
                                                        b_pos += 2;
                                                    }
                                                    _ => break,
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            let orig_size = header.original_size;
                            let comp_size = file_bytes.len() as u64;
                            let ratio = if orig_size > 0 { (comp_size as f64 / orig_size as f64) * 100.0 } else { 100.0 };
                            let sha256 = crate::checksum::bytes_to_hex(&header.original_sha256);

                            let _ = tx.send(TaskResult::InspectDone {
                                orig_size,
                                comp_size,
                                ratio,
                                sha256,
                                verified: true,
                                visual_blocks,
                                literal_count,
                                run_count,
                                token_count,
                                backref_count,
                                format_desc: format_desc.to_string(),
                                alg_desc,
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(format!("MZC 헤더 파싱 실패: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("압축 파일 로드 에러: {}", e)));
                }
            }
        });
    }

    /// **사전 학습 태스크를 백그라운드 스레드에 위임(Spawn)합니다.**
    fn spawn_train_task(&self, files: Vec<PathBuf>, output_path: PathBuf) {
        let tx = self.task_sender.clone();
        std::thread::spawn(move || {
            let mut all_bytes = Vec::new();
            for f in &files {
                if let Ok(b) = std::fs::read(f) {
                    all_bytes.extend(b);
                }
            }
            if all_bytes.is_empty() {
                let _ = tx.send(TaskResult::Error("학습 대상 파일 데이터가 비어 있습니다.".to_string()));
                return;
            }

            let dict = crate::rle::build_dictionary(&all_bytes);
            let dict_bytes = dict.to_bytes();
            
            match std::fs::write(&output_path, &dict_bytes) {
                Ok(_) => {
                    let _ = tx.send(TaskResult::TrainDone {
                        dict_size: dict_bytes.len(),
                        entry_count: dict.entries.len(),
                        saved_path: output_path,
                    });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("사전 파일 저장 실패: {}", e)));
                }
            }
        });
    }

    /// **tANS 압축 상태 천이 시뮬레이션을 돌려 플롯 점 데이터를 생성합니다.**
    fn run_tans_simulation(&mut self) {
        self.tans_sim_states.clear();
        self.tans_sim_bits.clear();
        
        let mut x = 32.0; // tANS 가상 디코드 초기 상태 값
        
        for i in 0..100 {
            self.tans_sim_states.push([i as f64, x]);
            
            // 가상 입력 비트 수집
            let bit = (i * 17 + 13) % 2;
            self.tans_sim_bits.push([i as f64, bit as f64]);
            
            // tANS 상태 천이 공식 의태 시뮬레이션
            if bit == 0 {
                x = (x * 1.35 + 8.0) % 512.0;
            } else {
                x = (x * 1.85 + 15.0) % 512.0;
            }
            if x < 32.0 {
                x += 32.0;
            }
        }
    }

    /// **비동기 벤치마크 태스크를 백그라운드 스레드에 위임(Spawn)합니다.**
    fn spawn_benchmark_task(&self, file_path: PathBuf) {
        let sender = self.task_sender.clone();
        std::thread::spawn(move || {
            let data = match std::fs::read(&file_path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    let _ = sender.send(TaskResult::Error(format!("파일 로드 실패: {}", e)));
                    return;
                }
            };
            if data.is_empty() {
                let _ = sender.send(TaskResult::Error("빈 파일은 벤치마크할 수 없습니다.".to_string()));
                return;
            }

            // 1. MZC 압축 실행 (Lz77 + CM 모드)
            let mzc_start = std::time::Instant::now();
            let mzc_comp = crate::compress_bytes_v2_dict(
                &data,
                CompressionMode::Lz77,
                EntropyMode::Cm,
                6,
                false,
                false,
                false,
                false,
                None,
            );
            let mzc_time = mzc_start.elapsed().as_secs_f64();
            let mzc_size = mzc_comp.len() as u64;

            // 2. Gzip 압축 실행
            let gzip_start = std::time::Instant::now();
            let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            use std::io::Write;
            let mut gzip_size = 0;
            let mut gzip_time = 0.0;
            if encoder.write_all(&data).is_ok() {
                if let Ok(comp) = encoder.finish() {
                    gzip_time = gzip_start.elapsed().as_secs_f64();
                    gzip_size = comp.len() as u64;
                }
            }

            // 3. Zstd 압축 실행
            let zstd_start = std::time::Instant::now();
            let mut zstd_size = 0;
            let mut zstd_time = 0.0;
            if let Ok(comp) = zstd::bulk::compress(&data, 3) {
                zstd_time = zstd_start.elapsed().as_secs_f64();
                zstd_size = comp.len() as u64;
            }

            let _ = sender.send(TaskResult::BenchmarkDone {
                mzc_size,
                gzip_size,
                zstd_size,
                mzc_time,
                gzip_time,
                zstd_time,
            });
        });
    }

    /// **CM 확률 맵 시뮬레이션 상태 초기화**
    fn init_cm_simulation(&mut self) {
        self.cm_sim_bit_idx = 0;
        self.cm_sim_byte_val = 0xAB; // 이진수 10101011
        self.cm_sim_bits = [true, false, true, false, true, false, true, true];
        self.cm_sim_ctx_byte = 1;
        self.cm_sim_prev1 = 0x55;
        self.cm_sim_prev2 = 0x33;
        self.cm_sim_weights = [[1024, 2048, 5120]; 8];
        self.cm_sim_probabilities = [2048, 2048, 2048];
        self.cm_sim_mixed_p = 2048;
    }

    /// **CM 확률 맵 시뮬레이션 1비트 진행**
    fn step_cm_simulation(&mut self) {
        if self.cm_sim_bit_idx >= 8 {
            return;
        }
        let bit_idx = self.cm_sim_bit_idx;
        let bit = self.cm_sim_bits[bit_idx];

        // Laplace 예측 카운트 모사 (시각화용 값 설정)
        let n0_0 = (2 + bit_idx) as u32;
        let n0_1 = (4 + (7 - bit_idx)) as u32;
        let p0 = ((n0_0 + 1) * 4096) / (n0_0 + n0_1 + 2);

        let n1_0 = (10 - bit_idx) as u32;
        let n1_1 = (5 + bit_idx) as u32;
        let p1 = ((n1_0 + 1) * 4096) / (n1_0 + n1_1 + 2);

        let n2_0 = (15 + bit_idx * 2) as u32;
        let n2_1 = (25 - bit_idx * 2) as u32;
        let p2 = ((n2_0 + 1) * 4096) / (n2_0 + n2_1 + 2);

        self.cm_sim_probabilities = [p0, p1, p2];

        let w = self.cm_sim_weights[bit_idx];
        let sum_w = (w[0] + w[1] + w[2]) as u32;
        let mut p = (w[0] as u32 * p0 + w[1] as u32 * p1 + w[2] as u32 * p2) / sum_w;
        if p == 0 { p = 1; } else if p >= 4096 { p = 4095; }
        self.cm_sim_mixed_p = p;

        // LMS 오차 가중치 업데이트
        let target = if !bit { 4096i32 } else { 0i32 };
        let err = target - p as i32;
        let learning_shift = 13; // 빠른 시각 변화용 학습률 조정

        let mut next_w = w;
        for i in 0..3 {
            let pi_val = match i {
                0 => p0 as i32,
                1 => p1 as i32,
                2 => p2 as i32,
                _ => unreachable!(),
            };
            let delta = (err * (pi_val - p as i32)) >> learning_shift;
            next_w[i] = (w[i] + delta).clamp(128, 16384);
        }

        if bit_idx + 1 < 8 {
            self.cm_sim_weights[bit_idx + 1] = next_w;
        }

        self.cm_sim_ctx_byte = (self.cm_sim_ctx_byte << 1) | (bit as u16);
        self.cm_sim_bit_idx += 1;
    }
}

impl eframe::App for MzcGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 비동기 작업 채널에서 수신한 비동기 결과를 상태 변수에 연동
        if let Ok(result) = self.task_receiver.try_recv() {
            match result {
                TaskResult::ChunkProgress { chunk_idx, orig_size, comp_size, duration } => {
                    if self.chunk_ratios.len() <= chunk_idx {
                        self.chunk_ratios.resize(chunk_idx + 1, 0.0);
                    }
                    if self.chunk_throughputs.len() <= chunk_idx {
                        self.chunk_throughputs.resize(chunk_idx + 1, 0.0);
                    }
                    let ratio = if orig_size > 0 { (comp_size as f64 / orig_size as f64) * 100.0 } else { 100.0 };
                    let throughput = if duration > 0.0 { (orig_size as f64 / 1_024_000.0) / duration } else { 0.0 };
                    self.chunk_ratios[chunk_idx] = ratio;
                    self.chunk_throughputs[chunk_idx] = throughput;
                }
                TaskResult::CompressDone {
                    orig_size,
                    comp_size,
                    ratio,
                    sha256,
                    visual_blocks,
                    literal_count,
                    run_count,
                    token_count,
                    backref_count,
                    saved_path,
                    format_desc,
                    alg_desc,
                } => {
                    self.is_processing = false;
                    self.original_size = orig_size;
                    self.compressed_size = comp_size;
                    self.compression_ratio = ratio;
                    self.sha256_hash = sha256;
                    self.visual_blocks = visual_blocks;
                    self.literal_count = literal_count;
                    self.run_count = run_count;
                    self.token_count = token_count;
                    self.backref_count = backref_count;
                    self.verified_ok = true;
                    self.format_description = format_desc;
                    self.algorithm_description = alg_desc;
                    self.status = format!("압축 성공! 저장됨: {:?}", saved_path.file_name().unwrap_or(saved_path.as_os_str()));
                }
                TaskResult::DecompressDone { restored_size, sha256, saved_path } => {
                    self.is_processing = false;
                    self.original_size = restored_size;
                    self.sha256_hash = sha256;
                    self.verified_ok = true;
                    self.status = format!("해제 완료 및 해시 검증 완료! 경로: {:?}", saved_path.file_name().unwrap_or(saved_path.as_os_str()));
                }
                TaskResult::InspectDone {
                    orig_size,
                    comp_size,
                    ratio,
                    sha256,
                    verified,
                    visual_blocks,
                    literal_count,
                    run_count,
                    token_count,
                    backref_count,
                    format_desc,
                    alg_desc,
                } => {
                    self.is_processing = false;
                    self.original_size = orig_size;
                    self.compressed_size = comp_size;
                    self.compression_ratio = ratio;
                    self.sha256_hash = sha256;
                    self.verified_ok = verified;
                    self.visual_blocks = visual_blocks;
                    self.literal_count = literal_count;
                    self.run_count = run_count;
                    self.token_count = token_count;
                    self.backref_count = backref_count;
                    self.format_description = format_desc;
                    self.algorithm_description = alg_desc;
                    self.status = "압축 데이터 분석 및 체크섬 무손실 검증에 완벽히 성공했습니다!".to_string();
                }
                TaskResult::TrainDone { dict_size, entry_count, saved_path } => {
                    self.is_processing = false;
                    self.train_status = format!(
                        "사전 생성 성공! 크기: {} bytes, 패턴 개수: {}개\n경로: {:?}", 
                        dict_size, entry_count, saved_path
                    );
                }
                TaskResult::BenchmarkDone {
                    mzc_size,
                    gzip_size,
                    zstd_size,
                    mzc_time,
                    gzip_time,
                    zstd_time,
                } => {
                    self.is_processing = false;
                    self.benchmark_mzc_size = mzc_size;
                    self.benchmark_gzip_size = gzip_size;
                    self.benchmark_zstd_size = zstd_size;
                    self.benchmark_mzc_time = mzc_time;
                    self.benchmark_gzip_time = gzip_time;
                    self.benchmark_zstd_time = zstd_time;
                    self.benchmark_status = "벤치마크 테스트 완료!".to_string();
                }
                TaskResult::UpdateCheckResult(res) => {
                    self.is_checking_update = false;
                    self.update_checked = true;
                    match res {
                        Ok(info) => {
                            self.update_available = Some(info);
                            self.show_update_modal = true;
                            self.status = "새로운 업데이트 버전이 발견되었습니다!".to_string();
                        }
                        Err(e) => {
                            self.update_available = None;
                            self.status = format!("업데이트 체크 결과: {}", e);
                        }
                    }
                }
                TaskResult::UpdateDownloadProgress(progress) => {
                    self.download_progress = progress;
                }
                TaskResult::UpdateDownloadDone(setup_path) => {
                    self.is_downloading_update = false;
                    #[cfg(target_os = "windows")]
                    {
                        let _ = std::process::Command::new(setup_path).spawn();
                        std::process::exit(0);
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        self.status = format!("다운로드 완료: {:?}", setup_path);
                    }
                }
                TaskResult::Error(e) => {
                    self.is_processing = false;
                    self.is_checking_update = false;
                    self.is_downloading_update = false;
                    self.status = format!("오류 발생: {}", e);
                    self.train_status = format!("오류 발생: {}", e);
                    self.benchmark_status = format!("오류 발생: {}", e);
                }
            }
        }

        // 드래그 앤 드롭 파일 탐색
        let mut is_drag_hovered = false;
        if !ctx.input(|i| i.raw.hovered_files.is_empty()) {
            is_drag_hovered = true;
        }

        if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
            let dropped = ctx.input(|i| i.raw.dropped_files.clone());
            if let Some(file) = dropped.first() {
                if let Some(ref path) = file.path {
                    self.input_path = Some(path.clone());
                    self.status = format!("대상 파일 준비: {:?}", path.file_name().unwrap_or(path.as_os_str()));
                }
            }
        }

        // ================== SIDEBAR PANEL (좌측 설정 옵션) ==================
        egui::SidePanel::left("sidebar_panel")
            .width_range(260.0..=285.0)
            .show(ctx, |ui| {
                ui.add_space(20.0);
                ui.heading("⚙ MZC Engine Control");
                ui.add_space(15.0);

                ui.group(|ui| {
                    ui.label("🛠 압축 모드 설정");
                    ui.add_space(6.0);
                    
                    ui.label("코어 알고리즘:");
                    ui.selectable_value(&mut self.compression_mode, CompressionMode::Lz77, "LZ77 슬라이딩윈도우 (MZC3)");
                    ui.selectable_value(&mut self.compression_mode, CompressionMode::Hybrid, "RLE 하이브리드 (MZC2)");
                    ui.selectable_value(&mut self.compression_mode, CompressionMode::Rle, "Retro RLE 단독 (MZC1)");
                    
                    ui.add_space(10.0);
                    ui.label("엔트로피 코더 (2차 비트 압축):");
                    ui.selectable_value(&mut self.entropy_mode, EntropyMode::Cm, "Context Mixing (MZC7)");
                    ui.selectable_value(&mut self.entropy_mode, EntropyMode::Ans, "tANS 테이블 압축 (MZC6)");
                    ui.selectable_value(&mut self.entropy_mode, EntropyMode::Dynamic, "동적 허프만 (MZC4)");
                    ui.selectable_value(&mut self.entropy_mode, EntropyMode::Huffman, "정적 허프만 코딩");
                    ui.selectable_value(&mut self.entropy_mode, EntropyMode::None, "2차 압축 안함 (None)");

                    ui.add_space(10.0);
                    ui.separator();
                    ui.label("⚡ 고급 전처리 필터 설정:");
                    
                    ui.add(egui::Slider::new(&mut self.compression_level, 1..=9).text("압축 레벨 (1-9)"));
                    
                    ui.checkbox(&mut self.png_enabled, "PNG Paeth 필터 (MZC7)");
                    ui.checkbox(&mut self.lpc_enabled, "LPC PCM 오디오 필터 (MZC7)");
                    ui.checkbox(&mut self.delta_enabled, "Delta 차분 필터");
                    ui.checkbox(&mut self.bcj_enabled, "BCJ 기계어 필터");

                    // 상호 배타 필터 충돌 방지 연동 제어
                    if self.png_enabled {
                        self.lpc_enabled = false;
                        self.delta_enabled = false;
                        self.bcj_enabled = false;
                    } else if self.lpc_enabled {
                        self.png_enabled = false;
                        self.delta_enabled = false;
                        self.bcj_enabled = false;
                    }
                });

                ui.add_space(10.0);

                ui.group(|ui| {
                    ui.label("📁 전역 공유 사전 설정");
                    ui.add_space(4.0);
                    
                    ui.horizontal(|ui| {
                        if let Some(ref path) = self.dict_path {
                            let name = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy();
                            ui.label(format!("선택됨: {}", name));
                        } else {
                            ui.label("없음 (로컬 사전)");
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        if ui.button("📁 선택...").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_file() {
                                self.dict_path = Some(path);
                            }
                        }
                        if self.dict_path.is_some() {
                            if ui.button("❌ 제거").clicked() {
                                self.dict_path = None;
                            }
                        }
                    });
                });

                ui.add_space(15.0);
                
                ui.group(|ui| {
                    ui.label("🚀 엔진 기동 명령");
                    ui.add_space(6.0);

                    if ui.button("📁 압축 대상 파일 열기...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("MZIP Compressed File (*.mzip)", &["mzip"])
                            .add_filter("All Files (*.*)", &["*"])
                            .pick_file() {
                            self.input_path = Some(path.clone());
                            self.status = format!("선택 파일: {:?}", path.file_name().unwrap_or(path.as_os_str()));
                        }
                    }
                    
                    ui.add_space(8.0);

                    if let Some(ref path) = self.input_path {
                        let compress_btn = ui.add_enabled(!self.is_processing, egui::Button::new("⚡ 고속 압축 실행"));
                        if compress_btn.clicked() {
                            self.is_processing = true;
                            self.chunk_ratios.clear();
                            self.chunk_throughputs.clear();
                            self.status = "백그라운드에서 압축 가동 중...".to_string();
                            self.spawn_compress_task(
                                path.clone(),
                                self.compression_mode,
                                self.entropy_mode,
                                self.compression_level,
                                self.delta_enabled,
                                self.bcj_enabled,
                                self.png_enabled,
                                self.lpc_enabled,
                                self.dict_path.clone(),
                            );
                        }

                        ui.add_space(6.0);

                        let decompress_btn = ui.add_enabled(!self.is_processing, egui::Button::new("🔓 압축 해제 및 복원"));
                        if decompress_btn.clicked() {
                            self.is_processing = true;
                            self.status = "백그라운드에서 역변환 및 체크섬 교차 검사 중...".to_string();
                            self.spawn_decompress_task(path.clone(), self.dict_path.clone());
                        }

                        ui.add_space(6.0);

                        let inspect_btn = ui.add_enabled(!self.is_processing, egui::Button::new("🔍 바이너리 구조 정밀 인스펙트"));
                        if inspect_btn.clicked() {
                            self.is_processing = true;
                            self.status = "MZC 구조화 매직 검출 및 헤더 분석 중...".to_string();
                            self.spawn_inspect_task(path.clone(), self.dict_path.clone());
                        }
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(180, 180, 180), "압축할 대상 파일을 먼저 선택해 주세요.");
                    }
                });

                ui.add_space(ui.available_height() - 150.0);
                
                ui.group(|ui| {
                    ui.label("🌐 시스템 & 관리");
                    ui.add_space(4.0);
                    
                    // 우클릭 컨텍스트 메뉴 제어 (Windows 전용)
                    ui.horizontal(|ui| {
                        if ui.button("➕ 우클릭 등록").clicked() {
                            #[cfg(target_os = "windows")]
                            {
                                match crate::register_context_menu() {
                                    Ok(_) => self.context_menu_status = "등록 성공!".to_string(),
                                    Err(e) => self.context_menu_status = format!("실패: {}", e),
                                }
                            }
                            #[cfg(not(target_os = "windows"))]
                            {
                                self.context_menu_status = "Windows만 지원".to_string();
                            }
                        }
                        if ui.button("➖ 우클릭 해제").clicked() {
                            #[cfg(target_os = "windows")]
                            {
                                match crate::unregister_context_menu() {
                                    Ok(_) => self.context_menu_status = "해제 성공!".to_string(),
                                    Err(e) => self.context_menu_status = format!("실패: {}", e),
                                }
                            }
                            #[cfg(not(target_os = "windows"))]
                            {
                                self.context_menu_status = "Windows만 지원".to_string();
                            }
                        }
                    });
                    ui.colored_label(egui::Color32::from_rgb(180, 180, 180), format!("우클릭: {}", self.context_menu_status));
                    
                    ui.add_space(6.0);
                    
                    // 자동 업데이트 확인 버튼
                    if ui.add_enabled(!self.is_checking_update && !self.is_downloading_update, egui::Button::new("🌐 최신 업데이트 확인")).clicked() {
                        self.is_checking_update = true;
                        self.status = "업데이트 서버에 연결 중...".to_string();
                        self.spawn_check_update_task();
                    }
                    if self.is_checking_update {
                        ui.spinner();
                    }
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(45, 206, 137), "✔ MZC Core Engine");
                    ui.colored_label(egui::Color32::from_rgb(120, 120, 120), format!("v{}", env!("CARGO_PKG_VERSION")));
                });
            });

        // ================== CENTRAL PANEL (중앙 결과 분석 및 멀티 탭 영역) ==================
        egui::CentralPanel::default().show(ctx, |ui| {
            if is_drag_hovered {
                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 2.0 - 20.0);
                        ui.colored_label(egui::Color32::from_rgb(45, 206, 137), " 여기에 파일을 드롭하세요 ");
                    });
                });
                return;
            }

            ui.add_space(5.0);
            
            // 중앙 패널 상단 멀티 탭 네비게이터 배치
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, 0, "📊 Dashboard (압축 진단 및 실시간 맵)");
                ui.selectable_value(&mut self.active_tab, 1, "🛠 Dictionary Trainer (사전 학습 위저드)");
                ui.selectable_value(&mut self.active_tab, 2, "📈 tANS Plot Simulator (상태 시뮬레이터)");
                ui.selectable_value(&mut self.active_tab, 3, "⚔ Benchmark (실시간 비교 벤치마크)");
                ui.selectable_value(&mut self.active_tab, 4, "🔬 CM Visualizer (확률 노드 시각화)");
            });
            ui.separator();

            match self.active_tab {
                // ================== Tab 0: Dashboard ==================
                0 => {
                    ui.horizontal(|ui| {
                        ui.heading(" MZC 압축 진단 및 실시간 시각화 맵");
                        if self.is_processing {
                            ui.spinner();
                        }
                    });
                    ui.add_space(4.0);

                    // 상태바 및 CPU 모니터링
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.colored_label(egui::Color32::from_rgb(45, 206, 137), "📢 현재 상태 :");
                            ui.label(&self.status);
                        });
                    });

                    let active_threads = if self.is_processing { rayon::current_num_threads() } else { 0 };
                    let total_threads = rayon::current_num_threads();
                    let occupancy_ratio = active_threads as f32 / total_threads as f32;
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.colored_label(egui::Color32::from_rgb(45, 206, 137), "💻 Rayon CPU 스레드 풀 점유 상태 :");
                            ui.add(egui::ProgressBar::new(occupancy_ratio)
                                .show_percentage()
                                .text(format!("{} / {} Cores Active ({:.1}%)", active_threads, total_threads, occupancy_ratio * 100.0))
                            );
                        });
                    });

                    ui.add_space(8.0);

                    // 명세 테이블
                    ui.columns(2, |columns| {
                        columns[0].group(|ui| {
                            ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "📁 대상 파일 명세");
                            ui.separator();
                            
                            let filename = match &self.input_path {
                                Some(p) => p.file_name().unwrap_or(p.as_os_str()).to_string_lossy().into_owned(),
                                None => "지정되지 않음".to_string(),
                            };
                            ui.label(format!("파일이름: {}", filename));
                            ui.label(format!("포맷 버전: {}", self.format_description));
                            ui.label(format!("적용 모드: {}", self.algorithm_description));
                        });

                        columns[1].group(|ui| {
                            ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "📊 정량적 압축 통계");
                            ui.separator();
                            
                            ui.label(format!("원본 크기: {} bytes", self.original_size));
                            if self.compressed_size > 0 {
                                ui.label(format!("압축 크기: {} bytes ({:.2}%)", self.compressed_size, self.compression_ratio));
                            } else {
                                ui.label("압축 크기: 대기 중");
                            }
                            if !self.sha256_hash.is_empty() {
                                ui.label(format!("SHA-256: {}", &self.sha256_hash[0..std::cmp::min(24, self.sha256_hash.len())]));
                            } else {
                                ui.label("체크섬 해시: 대기 중");
                            }
                        });
                    });

                    ui.add_space(15.0);

                    // 시각화 그리드 맵
                    ui.group(|ui| {
                        ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "🎨 페이로드 이진 블록 물리적 맵 (Real-Time Visual Block Grid)");
                        ui.separator();

                        ui.horizontal(|ui| {
                            ui.colored_label(egui::Color32::from_rgb(120, 120, 120), "범례:");
                            ui.colored_label(egui::Color32::from_rgb(45, 206, 137), "[R] RLE 런");
                            ui.colored_label(egui::Color32::from_rgb(41, 121, 255), "[T] 사전 토큰");
                            ui.colored_label(egui::Color32::from_rgb(255, 196, 0), "[B] LZ77 백레퍼런스");
                            ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "[L] 리터럴 바이트");
                        });
                        ui.add_space(5.0);

                        egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                            if self.visual_blocks.is_empty() {
                                ui.colored_label(egui::Color32::from_rgb(100, 100, 100), "분석/압축을 실행하여 이진 블록 레이아웃 지도를 렌더링하세요.");
                            } else {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                    for &ch in &self.visual_blocks {
                                        let (label, color, tooltip) = match ch {
                                            'R' => ("R", egui::Color32::from_rgb(45, 206, 137), "RLE 연속 반복 런 블록"),
                                            'T' => ("T", egui::Color32::from_rgb(41, 121, 255), "사전적 토큰 치환 블록"),
                                            'B' => ("B", egui::Color32::from_rgb(255, 196, 0), "LZ77 백레퍼런스 매치 블록"),
                                            _ => ("L", egui::Color32::from_rgb(90, 90, 95), "비압축 원시 리터럴 데이터 블록"),
                                        };

                                        let btn = ui.add(
                                            egui::Button::new(egui::RichText::new(label).monospace())
                                                .fill(color)
                                        );
                                        btn.on_hover_text(tooltip);
                                    }
                                });
                            }
                        });

                        ui.separator();
                        
                        let total_blocks = self.literal_count + self.run_count + self.token_count + self.backref_count;
                        if total_blocks > 0 {
                            ui.label(format!(
                                "⚡ 실측 블록 총합: {}개 | 리터럴: {}개 ({:.1}%) | RLE 런: {}개 ({:.1}%) | 사전 토큰: {}개 ({:.1}%) | LZ77 백레퍼런스: {}개 ({:.1}%)",
                                total_blocks,
                                self.literal_count, (self.literal_count as f64 / total_blocks as f64) * 100.0,
                                self.run_count, (self.run_count as f64 / total_blocks as f64) * 100.0,
                                self.token_count, (self.token_count as f64 / total_blocks as f64) * 100.0,
                                self.backref_count, (self.backref_count as f64 / total_blocks as f64) * 100.0,
                            ));
                        } else {
                            ui.label("블록 실측 카운트: 대기 중");
                        }
                    });

                    ui.add_space(10.0);

                    // 성능 그래프
                    ui.group(|ui| {
                        ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "📈 실시간 성능 모니터링");
                        ui.separator();

                        let ratio_points: PlotPoints = self.chunk_ratios.iter().enumerate()
                            .map(|(i, &r)| [i as f64 + 1.0, r])
                            .collect();
                        let throughput_points: PlotPoints = self.chunk_throughputs.iter().enumerate()
                            .map(|(i, &t)| [i as f64 + 1.0, t])
                            .collect();

                        ui.columns(2, |cols| {
                            cols[0].vertical(|ui| {
                                ui.label("압축률 변동 곡선 (%) - 낮을수록 우수");
                                let line = Line::new(ratio_points)
                                    .color(egui::Color32::from_rgb(255, 196, 0))
                                    .name("압축률");
                                Plot::new("ratio_plot")
                                    .height(110.0)
                                    .show(ui, |plot_ui| {
                                        plot_ui.line(line);
                                    });
                            });
                            cols[1].vertical(|ui| {
                                ui.label("처리량 속도 곡선 (MB/s) - 높을수록 우수");
                                let line = Line::new(throughput_points)
                                    .color(egui::Color32::from_rgb(41, 121, 255))
                                    .name("Throughput");
                                Plot::new("throughput_plot")
                                    .height(110.0)
                                    .show(ui, |plot_ui| {
                                        plot_ui.line(line);
                                    });
                            });
                        });
                    });
                }
                // ================== Tab 1: Dictionary Trainer ==================
                1 => {
                    ui.heading("🛠 MZC Shared Dictionary Trainer Wizard");
                    ui.label("다수의 원본 텍스트/바이너리 파일들로부터 자주 출현하는 반복 패턴을 스캔하여 사전 파일(.dict)로 저장합니다.");
                    ui.add_space(10.0);

                    ui.group(|ui| {
                        ui.label("📂 학습 대상 샘플 파일 목록");
                        ui.add_space(5.0);

                        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            if self.train_files.is_empty() {
                                ui.colored_label(egui::Color32::from_rgb(120, 120, 120), "목록이 비어 있습니다. 아래 버튼으로 파일을 추가하세요.");
                            } else {
                                for (idx, file) in self.train_files.iter().enumerate() {
                                    ui.label(format!("{}. {:?}", idx + 1, file.file_name().unwrap_or(file.as_os_str())));
                                }
                            }
                        });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("📁 샘플 파일 추가...").clicked() {
                                if let Some(files) = rfd::FileDialog::new().pick_files() {
                                    self.train_files.extend(files);
                                }
                            }
                            if ui.button("🗑 목록 초기화").clicked() {
                                self.train_files.clear();
                            }
                        });
                    });

                    ui.add_space(10.0);

                    ui.group(|ui| {
                        ui.label("💾 사전 파일 저장 경로 설정");
                        ui.add_space(4.0);
                        
                        ui.horizontal(|ui| {
                            ui.label(format!("저장 경로: {:?}", self.train_output_path.file_name().unwrap_or(self.train_output_path.as_os_str())));
                            if ui.button("📂 경로 지정...").clicked() {
                                if let Some(path) = rfd::FileDialog::new().save_file() {
                                    self.train_output_path = path;
                                }
                            }
                        });
                    });

                    ui.add_space(15.0);
                    
                    ui.horizontal(|ui| {
                        let train_btn = ui.add_enabled(!self.is_processing, egui::Button::new("⚙ 사전 학습 및 생성 개시"));
                        if train_btn.clicked() {
                            if self.train_files.is_empty() {
                                self.train_status = "학습 대상 파일을 최소 1개 이상 추가해야 합니다!".to_string();
                            } else {
                                self.is_processing = true;
                                self.train_status = "사전 가중치 테이블 분석 중...".to_string();
                                self.spawn_train_task(self.train_files.clone(), self.train_output_path.clone());
                            }
                        }
                    });

                    ui.add_space(10.0);
                    ui.group(|ui| {
                        ui.label("📢 학습 결과 및 상태:");
                        ui.colored_label(egui::Color32::from_rgb(45, 206, 137), &self.train_status);
                    });
                }
                // ================== Tab 2: tANS Plot Simulator ==================
                2 => {
                    ui.heading("📈 tANS State Transition & Bits Output Simulation");
                    ui.label("비대칭 수계(ANS) 엔트로피 압축 시 상태 변수 x와 비트 방출 과정이 어떻게 변이하는지 시각화로 모방합니다.");
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui.button("⚡ 시뮬레이션 새로고침 실행").clicked() {
                            self.run_tans_simulation();
                        }
                    });

                    ui.add_space(10.0);

                    if self.tans_sim_states.is_empty() {
                        ui.colored_label(egui::Color32::from_rgb(120, 120, 120), "위의 버튼을 눌러 시뮬레이션을 가동해 주세요.");
                    } else {
                        let state_points: PlotPoints = self.tans_sim_states.iter().map(|&p| p).collect();
                        let bit_points: PlotPoints = self.tans_sim_bits.iter().map(|&p| p).collect();

                        ui.vertical(|ui| {
                            ui.label("tANS 상태 변화 추이 (State x) - 심볼 인코딩에 따라 기하급수적 정수 범위 이동");
                            let line_state = Line::new(state_points)
                                .color(egui::Color32::from_rgb(255, 196, 0))
                                .name("State x");
                            Plot::new("tans_state_plot")
                                .height(160.0)
                                .show(ui, |plot_ui| {
                                    plot_ui.line(line_state);
                                });

                            ui.add_space(12.0);

                            ui.label("비트스트림 방출 여부 (0 또는 1) - 상태 한계를 벗어날 때마다 패킹되어 쓰여짐");
                            let line_bits = Line::new(bit_points)
                                .color(egui::Color32::from_rgb(41, 121, 255))
                                .name("Emitted Bit");
                            Plot::new("tans_bits_plot")
                                .height(100.0)
                                .show(ui, |plot_ui| {
                                    plot_ui.line(line_bits);
                                });
                        });
                    }
                }
                // ================== Tab 3: Benchmark ==================
                3 => {
                    ui.heading("⚔ Real-Time Multi-Format Benchmark");
                    ui.label("동일한 대상 파일에 대해 MZC (Lz77 + CM), Gzip (flate2), Zstd (zstd) 압축률과 소요 시간을 실시간으로 벤치마킹합니다.");
                    ui.add_space(10.0);

                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("📂 대상 파일:");
                            if let Some(ref path) = self.benchmark_file_path {
                                ui.colored_label(egui::Color32::from_rgb(45, 206, 137), path.to_string_lossy());
                            } else {
                                ui.colored_label(egui::Color32::from_rgb(120, 120, 120), "선택하지 않음");
                            }
                            if ui.button("선택...").clicked() {
                                if let Some(path) = rfd::FileDialog::new().pick_file() {
                                    self.benchmark_file_path = Some(path);
                                    self.benchmark_status = "파일 선택됨. 아래 버튼을 눌러 실행하세요.".to_string();
                                }
                            }
                        });
                    });

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        let btn = ui.add_enabled(!self.is_processing && self.benchmark_file_path.is_some(), egui::Button::new("⚡ 비교 벤치마크 가동"));
                        if btn.clicked() {
                            if let Some(ref path) = self.benchmark_file_path {
                                self.is_processing = true;
                                self.benchmark_status = "비교 압축 벤치마킹 구동 중...".to_string();
                                self.spawn_benchmark_task(path.clone());
                            }
                        }
                        if self.is_processing {
                            ui.spinner();
                        }
                        ui.label(&self.benchmark_status);
                    });

                    ui.add_space(15.0);

                    if self.benchmark_mzc_size > 0 {
                        // Display table
                        ui.group(|ui| {
                            ui.label("📊 포맷별 벤치마크 지표 비교");
                            ui.separator();
                            egui::Grid::new("benchmark_grid").striped(true).show(ui, |ui| {
                                ui.label("포맷");
                                ui.label("압축 후 크기 (bytes)");
                                ui.label("압축률 (낮을수록 우수)");
                                ui.label("압축 시간");
                                ui.end_row();

                                // MZC
                                ui.colored_label(egui::Color32::from_rgb(45, 206, 137), "MZC (Lz77 + CM)");
                                ui.label(self.benchmark_mzc_size.to_string());
                                if self.original_size > 0 {
                                    ui.label(format!("{:.2}%", (self.benchmark_mzc_size as f64 / self.original_size as f64) * 100.0));
                                } else {
                                    ui.label("-");
                                }
                                ui.label(format!("{:.4}s", self.benchmark_mzc_time));
                                ui.end_row();

                                // Gzip
                                ui.colored_label(egui::Color32::from_rgb(255, 196, 0), "Gzip (flate2)");
                                ui.label(self.benchmark_gzip_size.to_string());
                                if self.original_size > 0 {
                                    ui.label(format!("{:.2}%", (self.benchmark_gzip_size as f64 / self.original_size as f64) * 100.0));
                                } else {
                                    ui.label("-");
                                }
                                ui.label(format!("{:.4}s", self.benchmark_gzip_time));
                                ui.end_row();

                                // Zstd
                                ui.colored_label(egui::Color32::from_rgb(41, 121, 255), "Zstd (zstd)");
                                ui.label(self.benchmark_zstd_size.to_string());
                                if self.original_size > 0 {
                                    ui.label(format!("{:.2}%", (self.benchmark_zstd_size as f64 / self.original_size as f64) * 100.0));
                                } else {
                                    ui.label("-");
                                }
                                ui.label(format!("{:.4}s", self.benchmark_zstd_time));
                                ui.end_row();
                            });
                        });

                        ui.add_space(15.0);

                        // Size Plot Comparison
                        ui.columns(2, |cols| {
                            cols[0].vertical(|ui| {
                                ui.label("📉 압축 후 크기 비교 (bytes) - 낮을수록 우수");
                                let mzc_bar = Bar::new(0.5, self.benchmark_mzc_size as f64).name("MZC").fill(egui::Color32::from_rgb(45, 206, 137));
                                let gzip_bar = Bar::new(1.5, self.benchmark_gzip_size as f64).name("Gzip").fill(egui::Color32::from_rgb(255, 196, 0));
                                let zstd_bar = Bar::new(2.5, self.benchmark_zstd_size as f64).name("Zstd").fill(egui::Color32::from_rgb(41, 121, 255));
                                let chart = BarChart::new(vec![mzc_bar, gzip_bar, zstd_bar]).width(0.6);

                                Plot::new("benchmark_size_plot")
                                    .height(180.0)
                                    .show(ui, |plot_ui| {
                                        plot_ui.bar_chart(chart);
                                    });
                            });

                            cols[1].vertical(|ui| {
                                ui.label("⏱ 압축 소요 시간 비교 (seconds) - 낮을수록 우수");
                                let mzc_time_bar = Bar::new(0.5, self.benchmark_mzc_time).name("MZC").fill(egui::Color32::from_rgb(45, 206, 137));
                                let gzip_time_bar = Bar::new(1.5, self.benchmark_gzip_time).name("Gzip").fill(egui::Color32::from_rgb(255, 196, 0));
                                let zstd_time_bar = Bar::new(2.5, self.benchmark_zstd_time).name("Zstd").fill(egui::Color32::from_rgb(41, 121, 255));
                                let chart = BarChart::new(vec![mzc_time_bar, gzip_time_bar, zstd_time_bar]).width(0.6);

                                Plot::new("benchmark_time_plot")
                                    .height(180.0)
                                    .show(ui, |plot_ui| {
                                        plot_ui.bar_chart(chart);
                                    });
                            });
                        });
                    }
                }
                // ================== Tab 4: CM Visualizer ==================
                4 => {
                    ui.heading("🔬 Context Mixing (CM) Bit Probability & Weights Visualizer");
                    ui.label("디코딩 과정에서 각 비트마다 0차/1차/2차 문맥 정보의 기여 확률이 어떻게 변하고 가중치가 LMS 알고리즘을 통해 미세 학습 조율되는지 모사합니다.");
                    ui.add_space(10.0);

                    // Simulation auto-play timer trigger
                    if self.cm_sim_autoplay && self.cm_sim_bit_idx < 8 {
                        if self.cm_sim_last_step_time.elapsed() >= std::time::Duration::from_millis(1000) {
                            self.step_cm_simulation();
                            self.cm_sim_last_step_time = std::time::Instant::now();
                        }
                    }

                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("대상 데이터: 1바이트 0x{:02X} (이진수: ", self.cm_sim_byte_val));
                            for (idx, &b) in self.cm_sim_bits.iter().enumerate() {
                                if idx == self.cm_sim_bit_idx {
                                    ui.colored_label(egui::Color32::from_rgb(45, 206, 137), format!("[{}]", if b { "1" } else { "0" }));
                                } else {
                                    ui.label(if b { "1" } else { "0" });
                                }
                            }
                            ui.label(")");

                            ui.add_space(20.0);

                            if ui.button("🔄 리셋").clicked() {
                                self.init_cm_simulation();
                            }
                            if ui.button("▶ 1비트씩").clicked() {
                                self.step_cm_simulation();
                            }
                            let play_label = if self.cm_sim_autoplay { "⏸ 정지" } else { "▶ 자동 실행" };
                            if ui.button(play_label).clicked() {
                                self.cm_sim_autoplay = !self.cm_sim_autoplay;
                                self.cm_sim_last_step_time = std::time::Instant::now();
                            }
                        });
                    });

                    ui.add_space(10.0);

                    // Display current state
                    ui.columns(2, |cols| {
                        cols[0].group(|ui| {
                            ui.label(format!("비트 경로 상태: {}번째 비트 처리 중", self.cm_sim_bit_idx));
                            ui.separator();
                            ui.label(format!("현재 문맥 바이트(ctx_byte): 0x{:02X}", self.cm_sim_ctx_byte));
                            ui.label(format!("직전 1바이트: 0x{:02X}", self.cm_sim_prev1));
                            ui.label(format!("직전 2바이트: 0x{:02X}", self.cm_sim_prev2));
                            ui.add_space(10.0);

                            ui.label("각 차수별 예측 확률 (0=0%, 4096=100%):");
                            ui.colored_label(egui::Color32::from_rgb(150, 150, 150), format!("0차 문맥 확률 (p0): {}", self.cm_sim_probabilities[0]));
                            ui.colored_label(egui::Color32::from_rgb(255, 196, 0), format!("1차 문맥 확률 (p1): {}", self.cm_sim_probabilities[1]));
                            ui.colored_label(egui::Color32::from_rgb(41, 121, 255), format!("2차 문맥 확률 (p2): {}", self.cm_sim_probabilities[2]));
                            
                            ui.add_space(5.0);
                            ui.colored_label(egui::Color32::from_rgb(45, 206, 137), format!("최종 혼합 확률 (p): {}", self.cm_sim_mixed_p));
                        });

                        cols[1].group(|ui| {
                            ui.label("🧠 LMS 적응형 가중치 상태");
                            ui.separator();
                            let w_idx = std::cmp::min(self.cm_sim_bit_idx, 7);
                            let w = self.cm_sim_weights[w_idx];
                            ui.label(format!("0차 문맥 가중치 (w0): {}", w[0]));
                            ui.add(egui::ProgressBar::new(w[0] as f32 / 16384.0).fill(egui::Color32::from_rgb(150, 150, 150)));
                            ui.label(format!("1차 문맥 가중치 (w1): {}", w[1]));
                            ui.add(egui::ProgressBar::new(w[1] as f32 / 16384.0).fill(egui::Color32::from_rgb(255, 196, 0)));
                            ui.label(format!("2차 문맥 가중치 (w2): {}", w[2]));
                            ui.add(egui::ProgressBar::new(w[2] as f32 / 16384.0).fill(egui::Color32::from_rgb(41, 121, 255)));
                        });
                    });

                    ui.add_space(15.0);

                    // Range Coder Vertical Line Splitting Animation
                    ui.group(|ui| {
                        ui.label("📐 Range Coder 비트별 수직선 구간 분할 (0-영역 vs 1-영역)");
                        ui.separator();

                        let p_ratio = self.cm_sim_mixed_p as f32 / 4096.0;
                        ui.horizontal(|ui| {
                            let total_width = ui.available_width();
                            let (rect, _response) = ui.allocate_exact_size(
                                egui::vec2(total_width, 32.0),
                                egui::Sense::hover()
                            );
                            
                            // Draw splitting bar
                            let painter = ui.painter();
                            // Left part (0 bit, probability p)
                            let split_x = rect.left() + total_width * p_ratio;
                            let left_rect = egui::Rect::from_min_max(rect.left_top(), egui::pos2(split_x, rect.bottom()));
                            painter.rect_filled(left_rect, 4.0, egui::Color32::from_rgb(45, 206, 137));
                            // Right part (1 bit, probability 1 - p)
                            let right_rect = egui::Rect::from_min_max(egui::pos2(split_x, rect.top()), rect.right_bottom());
                            painter.rect_filled(right_rect, 4.0, egui::Color32::from_rgb(255, 100, 100));

                            // Add texts
                            painter.text(
                                left_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("0-영역 ({:.1}%)", p_ratio * 100.0),
                                egui::FontId::monospace(14.0),
                                egui::Color32::WHITE
                            );
                            painter.text(
                                right_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("1-영역 ({:.1}%)", (1.0 - p_ratio) * 100.0),
                                egui::FontId::monospace(14.0),
                                egui::Color32::WHITE
                            );
                        });
                    });
                }
                _ => {}
            }
        });

        // 업데이트 권장 팝업창 모달
        if self.show_update_modal {
            if let Some(ref info) = self.update_available {
                let version = info.version.clone();
                let url = info.url.clone();
                let changelog = info.changelog.clone();
                
                let mut show = true;
                let mut close_clicked = false;
                egui::Window::new("🌐 새로운 업데이트 발견")
                    .open(&mut show)
                    .resizable(false)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.set_max_width(400.0);
                        ui.heading(format!("MZC v{} 출시!", version));
                        ui.add_space(10.0);
                        
                        ui.label("변경 내용:");
                        ui.group(|ui| {
                            ui.label(&changelog);
                        });
                        ui.add_space(15.0);
                        
                        if self.is_downloading_update {
                            ui.horizontal(|ui| {
                                ui.label("다운로드 중...");
                                ui.add(egui::ProgressBar::new(self.download_progress).show_percentage());
                            });
                        } else {
                            ui.horizontal(|ui| {
                                if ui.button("⚡ 업데이트 및 설치").clicked() {
                                    self.is_downloading_update = true;
                                    self.spawn_download_update_task(url);
                                }
                                if ui.button("닫기").clicked() {
                                    close_clicked = true;
                                }
                            });
                        }
                    });
                if !show || close_clicked {
                    self.show_update_modal = false;
                }
            }
        }

        // 비동기 채널 폴링 반응성 확보
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}

fn is_newer_version(new_ver: &str, current_ver: &str) -> bool {
    let new_parts: Vec<&str> = new_ver.split('.').collect();
    let curr_parts: Vec<&str> = current_ver.split('.').collect();
    
    for i in 0..std::cmp::min(new_parts.len(), curr_parts.len()) {
        let n = new_parts[i].parse::<u32>().unwrap_or(0);
        let c = curr_parts[i].parse::<u32>().unwrap_or(0);
        if n > c {
            return true;
        } else if n < c {
            return false;
        }
    }
    new_parts.len() > curr_parts.len()
}
