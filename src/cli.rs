use std::path::PathBuf;
use clap::{Parser, Subcommand, ValueEnum};

/// MZC (Minimal Zip Concept) CLI 도구
///
/// # Rust 개념 설명:
/// - `#[derive(Parser)]`: clap crate가 제공하는 매크로로, 구조체의 필드를 분석하여
///   자동으로 명령줄 인자(CLI) 파서 코드를 빌드 타임에 생성해 줍니다.
/// - `#[command(...)]`: 프로그램 이름, 설명, 버전 등의 메타데이터를 CLI 도움말에 반영합니다.
#[derive(Parser, Debug)]
#[command(
    name = "mzc",
    author = "Antigravity",
    version = "0.3.0",
    about = "Minimal Zip Concept - RLE, 사전 및 LZ77 슬라이딩 윈도우 기반 하이브리드 무손실 압축 CLI 도구",
    long_about = "MZC는 압축 알고리즘의 원리를 쉽게 이해하고 직접 설계한 포맷으로 압축/해제를 구현해 보는 Rust 학습용 무손실 압축 CLI 도구입니다."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// 압축 알고리즘의 동작 방식 모드를 나타내는 열거형입니다.
///
/// # Rust 개념 설명:
/// - `#[derive(ValueEnum)]`: clap에서 이 enum의 각 값을 명령줄의 옵션 문자열로 직접 매핑해 줍니다.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionMode {
    /// RLE 단독 동작 모드 (MZC1과 완전 호환)
    Rle,
    /// 사전 단독 압축 모드 (토큰 및 리터럴만 사용)
    Dict,
    /// RLE + 사전 복합 하이브리드 압축 모드
    Hybrid,
    /// LZ77 슬라이딩 윈도우 기반 하이브리드 압축 모드 (MZC3 스펙)
    Lz77,
}

/// 엔트로피 코딩(Entropy Coding) 2차 비트 압축 방식을 정의하는 열거형입니다.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntropyMode {
    /// 2차 비트 압축 없음
    None,
    /// 비트 단위 정적 허프만 코딩 인코딩
    Huffman,
    /// 가변 헤더 적용 동적 허프만 코딩 (MZC4)
    Dynamic,
    /// Asymmetric Numeral Systems 테이블 압축 (MZC6)
    Ans,
}

/// MZC CLI에서 사용할 서브커맨드 목록을 나타내는 열거형(Enum)입니다.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 원본 파일을 MZC2/MZC5 하이브리드 압축 파일로 변환합니다.
    Compress {
        /// 압축할 원본 파일의 경로
        #[arg(value_name = "INPUT_FILE")]
        input_file: PathBuf,

        /// 생성할 압축 파일의 출력 경로
        #[arg(value_name = "OUTPUT_FILE")]
        output_file: PathBuf,

        /// 압축 알고리즘의 동작 모드 선택
        #[arg(long, value_enum, default_value_t = CompressionMode::Hybrid)]
        mode: CompressionMode,

        /// 2차 엔트로피 비트 압축 적용 모드 선택
        #[arg(long, value_enum, default_value_t = EntropyMode::Huffman)]
        entropy: EntropyMode,

        /// 압축 레벨 지정 (1: 가장 빠름, 9: 압축율 극대화)
        #[arg(long, short = 'l', default_value_t = 6)]
        level: u8,

        /// 델타 필터 전처리 적용 여부
        #[arg(long)]
        delta: bool,

        /// BCJ (Branch-Call-Jump) 기계어 주소 번역 필터 전처리 적용 여부
        #[arg(long)]
        bcj: bool,

        /// 전역 공유 사전 파일 경로 (옵션)
        #[arg(long = "dict-file", value_name = "DICT_FILE")]
        dict_file: Option<PathBuf>,
    },

    /// MZC 압축 파일을 읽어 원래 파일로 원상 복구하며, SHA-256 검증을 수행합니다.
    Decompress {
        /// 압축이 되어 있는 MZC 파일 경로
        #[arg(value_name = "INPUT_FILE")]
        input_file: PathBuf,

        /// 압축을 해제하여 복원해 낼 출력 경로
        #[arg(value_name = "OUTPUT_FILE")]
        output_file: PathBuf,

        /// 전역 공유 사전 파일 경로 (옵션)
        #[arg(long = "dict-file", value_name = "DICT_FILE")]
        dict_file: Option<PathBuf>,
    },

    /// 지정한 원본 파일을 임시 메모리 내에서 압축 후 다시 해제하여 원본과 100% 동일한지 라운드트립 검증을 수행합니다.
    Test {
        /// 무손실 압축 검증을 수행해 볼 원본 파일 경로
        #[arg(value_name = "INPUT_FILE")]
        input_file: PathBuf,

        /// 압축 모드 선택
        #[arg(long, value_enum, default_value_t = CompressionMode::Hybrid)]
        mode: CompressionMode,

        /// 엔트로피 압축 모드 선택
        #[arg(long, value_enum, default_value_t = EntropyMode::Huffman)]
        entropy: EntropyMode,

        /// 압축 레벨 지정 (1: 가장 빠름, 9: 압축율 극대화)
        #[arg(long, short = 'l', default_value_t = 6)]
        level: u8,

        /// 델타 필터 전처리 적용 여부
        #[arg(long)]
        delta: bool,

        /// BCJ 필터 전처리 적용 여부
        #[arg(long)]
        bcj: bool,

        /// 전역 공유 사전 파일 경로 (옵션)
        #[arg(long = "dict-file", value_name = "DICT_FILE")]
        dict_file: Option<PathBuf>,
    },

    /// 다수의 원본 텍스트/바이너리 샘플로부터 공유 사전을 생성하여 파일로 저장합니다.
    Train {
        /// 사전에 학습할 다수 샘플 파일 경로
        #[arg(value_name = "INPUT_FILES", required = true)]
        input_files: Vec<PathBuf>,

        /// 저장할 사전 파일 출력 경로 (예: trained.dict)
        #[arg(short = 'o', long, value_name = "OUTPUT_FILE", default_value = "trained.dict")]
        output: PathBuf,
    },

    /// MZC 압축 파일을 읽어 헤더 명세와 압축율, 그리고 내장된 SHA-256 해시를 상세히 분석하여 출력합니다.
    Inspect {
        /// 분석을 수행할 대상 MZC 압축 파일 경로
        #[arg(value_name = "INPUT_FILE")]
        input_file: PathBuf,
    },

    /// MZC 그래픽 데스크톱 앱(GUI)을 실행합니다.
    Gui,
}
