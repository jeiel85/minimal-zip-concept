use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender, Receiver};
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use crate::cli::{CompressionMode, EntropyMode};
use crate::checksum::calculate_sha256;
use crate::rle::Dictionary;
use crate::huffman::{huffman_decompress, huffman_decompress_dynamic};
use crate::format::{
    MzcHeader, VERSION_MZC1, VERSION_MZC2, VERSION_MZC3, VERSION_MZC4, VERSION_MZC5,
    FILTER_DELTA, FILTER_BCJ, FILTER_DYNAMIC_HUFFMAN, ALGORITHM_RLE, ALGORITHM_DICT,
    ALGORITHM_HYBRID, ALGORITHM_LZ77, HEADER_SIZE_MZC1, HEADER_SIZE_MZC2
};

/// MZC 그래픽 데스크톱 GUI 애플리케이션을 구동합니다.
pub fn run_gui_app() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MZC (Minimal Zip Concept) - Advanced Interactive Compressor")
            .with_inner_size([920.0, 700.0])
            .with_min_inner_size([800.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "MZC Desktop App",
        options,
        Box::new(|cc| Box::new(MzcGuiApp::new(cc))),
    )
}

/// GUI 비동기 스레드 작업 결과를 수집하기 위한 채널 데이터 열거형입니다.
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
    Error(String),
}

/// MZC Desktop GUI의 코어 애플리케이션 상태를 관리하는 구조체입니다.
pub struct MzcGuiApp {
    // 인풋 설정
    input_path: Option<PathBuf>,
    compression_mode: CompressionMode,
    entropy_mode: EntropyMode,

    // 비동기 스레드 채널 및 처리 상태
    status: String,
    is_processing: bool,
    task_sender: Sender<TaskResult>,
    task_receiver: Receiver<TaskResult>,

    // 메타데이터 요약 및 통계
    original_size: u64,
    compressed_size: u64,
    compression_ratio: f64,
    sha256_hash: String,
    verified_ok: bool,

    // 포맷 상세 정보
    format_description: String,
    algorithm_description: String,

    // 블록 시각화 맵 데이터
    visual_blocks: Vec<char>,
    literal_count: usize,
    run_count: usize,
    token_count: usize,
    backref_count: usize,

    // MZC5 추가 설정
    compression_level: u8,
    delta_enabled: bool,
    bcj_enabled: bool,

    // 실시간 모니터링 그래프 플롯용 데이터
    chunk_ratios: Vec<f64>,
    chunk_throughputs: Vec<f64>,
}

impl MzcGuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 프리미엄 다크 슬레이트 테마의 Visuals를 수립합니다.
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = 12.0.into();
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(18, 18, 20); // 딥 다크 블랙
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(30, 30, 35); // 다크 그레이
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(45, 45, 52);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(45, 206, 137); // MZC 핵심 포인트 그린 컬러 (HSL 맞춤)
        cc.egui_ctx.set_visuals(visuals);

        let (task_sender, task_receiver) = channel();

        Self {
            input_path: None,
            compression_mode: CompressionMode::Lz77,
            entropy_mode: EntropyMode::Huffman,
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
            chunk_ratios: Vec::new(),
            chunk_throughputs: Vec::new(),
        }
    }

    /// 파일 압축 비동기 스레드 기동
    fn spawn_compress_task(&self, path: PathBuf, mode: CompressionMode, entropy: EntropyMode, level: u8, delta: bool, bcj: bool) {
        let tx = self.task_sender.clone();
        std::thread::spawn(move || {
            match std::fs::read(&path) {
                Ok(original_bytes) => {
                    let orig_size = original_bytes.len() as u64;
                    let sha256 = crate::checksum::bytes_to_hex(&calculate_sha256(&original_bytes));
                    
                    // 핵심 압축 연산 수행
                    let tx_progress = tx.clone();
                    let final_output = crate::compress_bytes_v2_with_progress(
                        &original_bytes,
                        mode,
                        entropy,
                        level,
                        delta,
                        bcj,
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

                    // 시각화 맵 수집을 위한 헤더 디코딩 분기 파싱
                    let mut visual_blocks = Vec::new();
                    let mut literal_count = 0;
                    let mut run_count = 0;
                    let mut token_count = 0;
                    let mut backref_count = 0;

                    let mut version_mzc5 = false;
                    if let Ok(header) = MzcHeader::from_bytes(&final_output) {
                        version_mzc5 = header.version == VERSION_MZC5;
                        let header_size = if header.version == VERSION_MZC2 || header.version == VERSION_MZC3 || header.version == VERSION_MZC4 || header.version == VERSION_MZC5 {
                            HEADER_SIZE_MZC2
                        } else {
                            HEADER_SIZE_MZC1
                        };
                        let payload_bytes = &final_output[header_size..];

                        if header.version == VERSION_MZC2 || header.version == VERSION_MZC3 || header.version == VERSION_MZC4 {
                            let mut pos = 0;
                            let n = payload_bytes.len();
                            while pos < n {
                                if pos + 12 > n { break; }
                                let comb_size = u32::from_le_bytes(payload_bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
                                let comp_size = u32::from_le_bytes(payload_bytes[pos + 8..pos + 12].try_into().unwrap()) as usize;
                                pos += 12;
                                if pos + comp_size > n { break; }

                                let chunk_data = &payload_bytes[pos..pos + comp_size];
                                pos += comp_size;

                                let unhuff = if header.version == VERSION_MZC4 {
                                    huffman_decompress_dynamic(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                } else if chunk_data.len() != comb_size {
                                    huffman_decompress(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                } else {
                                    chunk_data.to_vec()
                                };

                                let dict = Dictionary::from_bytes(&unhuff).unwrap_or_default();
                                let dict_bytes_len = dict.to_bytes().len();
                                if dict_bytes_len >= unhuff.len() { continue; }
                                let rle_payload = &unhuff[dict_bytes_len..];

                                let mut b_pos = 0;
                                let b_n = rle_payload.len();
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
                        } else if header.version == VERSION_MZC5 {
                            let mut pos = 0;
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

                                let is_dynamic = (header.algorithm_type & FILTER_DYNAMIC_HUFFMAN) != 0;
                                let unhuff = if is_dynamic {
                                    huffman_decompress_dynamic(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                } else if chunk_data.len() != comb_size {
                                    huffman_decompress(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                } else {
                                    chunk_data.to_vec()
                                };

                                let dict = Dictionary::from_bytes(&unhuff).unwrap_or_default();
                                let dict_bytes_len = dict.to_bytes().len();
                                if dict_bytes_len < unhuff.len() {
                                    let rle_payload = &unhuff[dict_bytes_len..];
                                    let mut b_pos = 0;
                                    let b_n = rle_payload.len();
                                    let mut decomp_size = 0;
                                    
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
                                                    let token_idx = u16::from_le_bytes(rle_payload[b_pos..b_pos + 2].try_into().unwrap()) as usize;
                                                    b_pos += 2;
                                                    let entry_len = if token_idx < dict.entries.len() {
                                                        dict.entries[token_idx].len()
                                                    } else {
                                                        0
                                                    };
                                                    decomp_size += entry_len;
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
                                }
                            }
                        }
                    }

                    // 파일 쓰기 진행
                    let mut saved_path = path.clone();
                    let ext = if version_mzc5 {
                        "mzc5"
                    } else if entropy == EntropyMode::Dynamic {
                        "mzc4"
                    } else {
                        match mode {
                            CompressionMode::Rle => "mzc1",
                            CompressionMode::Dict => "mzc2",
                            CompressionMode::Hybrid => "mzc2",
                            CompressionMode::Lz77 => "mzc3",
                        }
                    };
                    saved_path.set_extension(ext);

                    let format_desc = if version_mzc5 {
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
                            let _ = tx.send(TaskResult::Error(format!("압축 파일 쓰기 오류: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("원본 파일 읽기 오류: {}", e)));
                }
            }
        });
    }

    /// 파일 압축 해제 비동기 스레드 기동
    fn spawn_decompress_task(&self, path: PathBuf) {
        let tx = self.task_sender.clone();
        std::thread::spawn(move || {
            match std::fs::read(&path) {
                Ok(compressed_bytes) => {
                    match crate::decompress_bytes_v2(&compressed_bytes) {
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
                                    let _ = tx.send(TaskResult::Error(format!("복원 파일 쓰기 오류: {}", e)));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(format!("압축 해제 처리 오류: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("압축 파일 로드 오류: {}", e)));
                }
            }
        });
    }

    /// 파일 헤더 상세 인스펙트 비동기 스레드 기동
    fn spawn_inspect_task(&self, path: PathBuf) {
        let tx = self.task_sender.clone();
        std::thread::spawn(move || {
            match std::fs::read(&path) {
                Ok(file_bytes) => {
                    match MzcHeader::from_bytes(&file_bytes) {
                        Ok(header) => {
                            let header_size = if header.version == VERSION_MZC2 || header.version == VERSION_MZC3 || header.version == VERSION_MZC4 || header.version == VERSION_MZC5 {
                                HEADER_SIZE_MZC2
                            } else {
                                HEADER_SIZE_MZC1
                            };

                            let format_desc = if header.version == VERSION_MZC5 {
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

                            let core_alg = if header.version == VERSION_MZC5 {
                                header.algorithm_type & 0x0F
                            } else {
                                header.algorithm_type
                            };

                            let alg_desc = if header.version == VERSION_MZC5 {
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

                            if (header.version == VERSION_MZC2 || header.version == VERSION_MZC3 || header.version == VERSION_MZC4) && header.original_size > 0 {
                                let mut pos = 0;
                                let n = payload_bytes.len();
                                while pos < n {
                                    if pos + 12 > n { break; }
                                    let comb_size = u32::from_le_bytes(payload_bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
                                    let comp_size = u32::from_le_bytes(payload_bytes[pos + 8..pos + 12].try_into().unwrap()) as usize;
                                    pos += 12;
                                    if pos + comp_size > n { break; }

                                    let chunk_data = &payload_bytes[pos..pos + comp_size];
                                    pos += comp_size;

                                    let unhuff = if header.version == VERSION_MZC4 {
                                        huffman_decompress_dynamic(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                    } else if chunk_data.len() != comb_size {
                                        huffman_decompress(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                    } else {
                                        chunk_data.to_vec()
                                    };

                                    let dict = Dictionary::from_bytes(&unhuff).unwrap_or_default();
                                    let dict_bytes_len = dict.to_bytes().len();
                                    if dict_bytes_len >= unhuff.len() { continue; }
                                    let rle_payload = &unhuff[dict_bytes_len..];

                                    let mut b_pos = 0;
                                    let b_n = rle_payload.len();
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
                            } else if header.version == VERSION_MZC5 && header.original_size > 0 {
                                let mut pos = 0;
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

                                    let is_dynamic = (header.algorithm_type & FILTER_DYNAMIC_HUFFMAN) != 0;
                                    let unhuff = if is_dynamic {
                                        huffman_decompress_dynamic(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                    } else if chunk_data.len() != comb_size {
                                        huffman_decompress(chunk_data, comb_size).unwrap_or_else(|_| chunk_data.to_vec())
                                    } else {
                                        chunk_data.to_vec()
                                    };

                                    let dict = Dictionary::from_bytes(&unhuff).unwrap_or_default();
                                    let dict_bytes_len = dict.to_bytes().len();
                                    if dict_bytes_len < unhuff.len() {
                                        let rle_payload = &unhuff[dict_bytes_len..];
                                        let mut b_pos = 0;
                                        let b_n = rle_payload.len();
                                        let mut decomp_size = 0;
                                        
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
                                                        let token_idx = u16::from_le_bytes(rle_payload[b_pos..b_pos + 2].try_into().unwrap()) as usize;
                                                        b_pos += 2;
                                                        let entry_len = if token_idx < dict.entries.len() {
                                                            dict.entries[token_idx].len()
                                                        } else {
                                                            0
                                                        };
                                                        decomp_size += entry_len;
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
                                    }
                                }
                            } else if header.version == VERSION_MZC1 && header.original_size > 0 {
                                let mut b_pos = 0;
                                let b_n = payload_bytes.len();
                                while b_pos < b_n {
                                    if b_pos + 3 > b_n { break; }
                                    let b_type = payload_bytes[b_pos];
                                    let b_len = u16::from_le_bytes(payload_bytes[b_pos + 1..b_pos + 3].try_into().unwrap()) as usize;
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
                                        _ => break,
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
                                alg_desc: alg_desc.to_string(),
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(format!("MZC 헤더 판독 실패: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("파일 로드 오류: {}", e)));
                }
            }
        });
    }
}

impl eframe::App for MzcGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 비동기 워커 스레드의 완료 채널을 실시간 수집하여 상태 변수에 동기화
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
                    self.status = format!("압축 완료! 파일이 성공적으로 저장되었습니다: {:?}", saved_path.file_name().unwrap_or(saved_path.as_os_str()));
                }
                TaskResult::DecompressDone { restored_size, sha256, saved_path } => {
                    self.is_processing = false;
                    self.original_size = restored_size;
                    self.sha256_hash = sha256;
                    self.verified_ok = true;
                    self.status = format!("압축 해제 및 SHA-256 검증 성공! 파일 저장 경로: {:?}", saved_path.file_name().unwrap_or(saved_path.as_os_str()));
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
                    self.status = "압축 파일 검증 및 분석(Inspect)에 성공하였습니다!".to_string();
                }
                TaskResult::Error(e) => {
                    self.is_processing = false;
                    self.status = format!("오류 발생: {}", e);
                }
            }
        }

        // 마운트 드래그 앤 드롭 파일 탐지 로직
        let mut is_drag_hovered = false;
        if !ctx.input(|i| i.raw.hovered_files.is_empty()) {
            is_drag_hovered = true;
        }

        if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
            let dropped = ctx.input(|i| i.raw.dropped_files.clone());
            if let Some(file) = dropped.first() {
                if let Some(ref path) = file.path {
                    self.input_path = Some(path.clone());
                    self.status = format!("대상 파일 준비 완료: {:?}", path.file_name().unwrap_or(path.as_os_str()));
                }
            }
        }

        // ================== SIDEBAR PANEL (좌측 설정 영역) ==================
        egui::SidePanel::left("sidebar_panel")
            .width_range(260.0..=280.0)
            .show(ctx, |ui| {
                ui.add_space(20.0);
                ui.heading(" MZC Dashboard");
                ui.add_space(15.0);

                ui.group(|ui| {
                    ui.label("⚙ 압축 옵션 설정");
                    ui.add_space(8.0);
                    
                    ui.label("알고리즘 모드:");
                    ui.selectable_value(&mut self.compression_mode, CompressionMode::Lz77, "LZ77 하이브리드 (MZC3)");
                    ui.selectable_value(&mut self.compression_mode, CompressionMode::Hybrid, "RLE 하이브리드 (MZC2)");
                    ui.selectable_value(&mut self.compression_mode, CompressionMode::Rle, "RLE 단독형 (MZC1)");
                    
                    ui.add_space(10.0);
                    ui.label("엔트로피 코딩 (2차 비트 압축):");
                    ui.selectable_value(&mut self.entropy_mode, EntropyMode::Dynamic, "동적 허프만 코딩 (MZC4)");
                    ui.selectable_value(&mut self.entropy_mode, EntropyMode::Huffman, "정적 허프만 코딩");
                    ui.selectable_value(&mut self.entropy_mode, EntropyMode::None, "2차 압축 해제 (None)");

                    ui.add_space(10.0);
                    ui.separator();
                    ui.label("⚡ MZC5 고급 최적화 설정:");
                    
                    ui.add(egui::Slider::new(&mut self.compression_level, 1..=9).text("압축 레벨 (1-9)"));
                    ui.checkbox(&mut self.delta_enabled, "Delta 필터 활성화");
                    ui.checkbox(&mut self.bcj_enabled, "BCJ 필터 활성화");
                });

                ui.add_space(20.0);
                
                ui.group(|ui| {
                    ui.label("🛠 압축 작업 실행");
                    ui.add_space(8.0);

                    // 파일 직접 열기 버튼
                    if ui.button("📁 파일 열기...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.input_path = Some(path.clone());
                            self.status = format!("선택된 파일: {:?}", path.file_name().unwrap_or(path.as_os_str()));
                        }
                    }
                    
                    ui.add_space(8.0);

                    if let Some(ref path) = self.input_path {
                        // 압축
                        let compress_btn = ui.add_enabled(!self.is_processing, egui::Button::new("⚡ 고속 압축 실행"));
                        if compress_btn.clicked() {
                            self.is_processing = true;
                            self.chunk_ratios.clear();
                            self.chunk_throughputs.clear();
                            self.status = "백그라운드 스레드에서 압축 및 패킹 진행 중...".to_string();
                            self.spawn_compress_task(
                                path.clone(),
                                self.compression_mode,
                                self.entropy_mode,
                                self.compression_level,
                                self.delta_enabled,
                                self.bcj_enabled,
                            );
                        }

                        ui.add_space(6.0);

                        // 해제
                        let decompress_btn = ui.add_enabled(!self.is_processing, egui::Button::new("🔓 압축 해제 및 복원"));
                        if decompress_btn.clicked() {
                            self.is_processing = true;
                            self.status = "백그라운드 스레드에서 무손실 해제 검증 진행 중...".to_string();
                            self.spawn_decompress_task(path.clone());
                        }

                        ui.add_space(6.0);

                        // 인스펙트
                        let inspect_btn = ui.add_enabled(!self.is_processing, egui::Button::new("🔍 압축 파일 정밀 검사"));
                        if inspect_btn.clicked() {
                            self.is_processing = true;
                            self.status = "페이로드 이진 스트림 정밀 파싱 중...".to_string();
                            self.spawn_inspect_task(path.clone());
                        }
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(180, 180, 180), "압축할 대상 파일을 선택하거나 마우스로 끌어서 창 위에 놓아주세요.");
                    }
                });

                ui.add_space(ui.available_height() - 40.0);
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(45, 206, 137), "✔ MZC Desktop Engine");
                    ui.colored_label(egui::Color32::from_rgb(120, 120, 120), "v3.0.0");
                });
            });

        // ================== CENTRAL PANEL (중앙 결과 분석 및 드롭 영역) ==================
        egui::CentralPanel::default().show(ctx, |ui| {
            // 드래그 호버 오버레이 연출
            if is_drag_hovered {
                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 2.0 - 20.0);
                        ui.colored_label(egui::Color32::from_rgb(45, 206, 137), " 여기에 파일을 끌어다 놓으세요 ");
                    });
                });
                return;
            }

            ui.add_space(5.0);
            
            // 프리미엄 상단 헤더
            ui.horizontal(|ui| {
                ui.heading(" MZC 압축 진단 및 실시간 시각화 맵");
                if self.is_processing {
                    ui.spinner();
                }
            });
            ui.separator();

            // 실시간 상태 표시란 (HSL 하이라이트 배경 그룹)
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(45, 206, 137), "📢 현재 상태 :");
                    ui.label(&self.status);
                });
            });

            // Rayon CPU 스레드 풀 점유 게이지
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

            // 파일 정보 & 실시간 메타데이터 카드 배치 (2단 배치)
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

            // ================== 압축 블록 시각화 맵 영역 (Vibrant block map) ==================
            ui.group(|ui| {
                ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "🎨 페이로드 이진 블록 물리적 맵 (Real-Time Visual Block Grid Canvas)");
                ui.separator();

                // 범례 표시란
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(120, 120, 120), "범례:");
                    ui.colored_label(egui::Color32::from_rgb(45, 206, 137), "[R] RLE 런");
                    ui.colored_label(egui::Color32::from_rgb(41, 121, 255), "[T] 사전 토큰");
                    ui.colored_label(egui::Color32::from_rgb(255, 196, 0), "[B] LZ77 백레퍼런스");
                    ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "[L] 리터럴 바이트");
                });
                ui.add_space(5.0);

                // 블록 그리드 맵을 스크롤 영역에 렌더링
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    if self.visual_blocks.is_empty() {
                        ui.colored_label(egui::Color32::from_rgb(100, 100, 100), "분석/압축을 실행하여 이진 블록 레이아웃 지도를 띄우세요.");
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            // 그리드 크기를 설정하여 세로 정렬이 맞춰지도록 유도
                            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                            for &ch in &self.visual_blocks {
                                // 블록 유형에 맞춰 HSL 테마에 부합하는 컬러로 사각 rounded 버튼 그리기
                                let (label, color, tooltip) = match ch {
                                    'R' => ("R", egui::Color32::from_rgb(45, 206, 137), "RLE 연속 반복 블록 (Green Run Block)"),
                                    'T' => ("T", egui::Color32::from_rgb(41, 121, 255), "사전적 토큰 치환 블록 (Blue Token Block)"),
                                    'B' => ("B", egui::Color32::from_rgb(255, 196, 0), "LZ77 슬라이딩 윈도우 백레퍼런스 (Yellow BackRef Block)"),
                                    _ => ("L", egui::Color32::from_rgb(90, 90, 95), "비압축 원시 데이터 블록 (Grey Literal Block)"),
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
                
                // 블록 개수 실측 카운트 요약 바
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

            // 실시간 성능 그래프 대시보드
            ui.group(|ui| {
                ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "📈 실시간 성능 모니터링 (Throughput & Compression Ratio curves)");
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
                            .height(130.0)
                            .show(ui, |plot_ui| {
                                plot_ui.line(line);
                            });
                    });
                    cols[1].vertical(|ui| {
                        ui.label("처리 처리량 속도 곡선 (MB/s) - 높을수록 우수");
                        let line = Line::new(throughput_points)
                            .color(egui::Color32::from_rgb(41, 121, 255))
                            .name("Throughput");
                        Plot::new("throughput_plot")
                            .height(130.0)
                            .show(ui, |plot_ui| {
                                plot_ui.line(line);
                            });
                    });
                });
            });
        });

        // 프레임을 주기적으로 리페인트하여 비동기 스레드 상태 전이가 즉각 반응하도록 보장합니다.
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}
