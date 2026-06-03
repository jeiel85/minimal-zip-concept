use std::fmt;

/// MZC 프로젝트에서 발생하는 다양한 에러들을 정의하는 열거형(Enum)입니다.
#[derive(Debug)]
pub enum MzcError {
    /// 최소 헤더 크기를 충족하지 못하고 파일이 잘렸을 때 발생합니다.
    TruncatedHeader { read_bytes: usize },

    /// Magic Header가 유효하지 않을 때 발생합니다.
    InvalidMagic { expected: String, found: String },

    /// MZC 포맷 버전이 유효하지 않을 때 발생합니다.
    InvalidVersion { expected: u8, found: u8 },

    /// 알고리즘 타입이 유효하지 않을 때 발생합니다.
    InvalidAlgorithm { expected: u8, found: u8 },

    /// 디코딩 중 규정되지 않은 잘못된 블록 타입을 만났을 때 발생합니다.
    UnknownBlockType { found: u8 },

    /// 블록 데이터가 비정상적으로 잘렸을 때 발생합니다.
    TruncatedBlock { expected: usize, found: usize },

    /// 디코딩한 결과 데이터의 최종 크기가 헤더의 Original Size와 일치하지 않을 때 발생합니다.
    OriginalSizeMismatch { expected: u64, found: u64 },

    /// 디코딩된 데이터의 SHA-256 체크섬이 헤더에 저장된 원본 해시와 다를 때 발생합니다. (무손실 실패)
    ChecksumMismatch {
        expected: String,
        found: String,
    },

    /// MZC2 사전 섹션을 파싱하는 도중 데이터가 잘렸거나 레이아웃이 손상되었을 때 발생합니다.
    CorruptDictionary,

    /// 디코딩된 토큰 블록의 인덱스가 실제 사전에 수록된 단어 수 이상을 주목할 때 발생합니다.
    InvalidTokenIndex { index: u16, max_valid: u16 },

    /// 허프만 압축 해제 중 잘못된 코드나 잘린 비트스트림을 감지했을 때 발생합니다.
    HuffmanError { message: String },

    /// LZ77 디코딩 시 유효 범위를 벗어나는 백레퍼런스 참조를 감지했을 때 발생합니다.
    InvalidBackRef { distance: u16, length: u16, current_size: usize },
}

// Rust에서 사용자 정의 에러를 표준 출력용 포맷으로 만들기 위해 Display 트레이트(Trait)를 구현합니다.
impl fmt::Display for MzcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MzcError::TruncatedHeader { read_bytes } => {
                write!(f, "파일 헤더가 손상되었습니다. 필요한 헤더 바이트가 부족하며, {read_bytes}바이트만 읽었습니다.")
            }
            MzcError::InvalidMagic { expected, found } => {
                write!(f, "잘못된 파일 형식 (Magic Header). 기대치: '{expected}', 실제: '{found}'")
            }
            MzcError::InvalidVersion { expected, found } => {
                write!(f, "지원하지 않는 버전입니다. 기대치: {expected:#04x}, 실제: {found:#04x}")
            }
            MzcError::InvalidAlgorithm { expected, found } => {
                write!(f, "지원하지 않는 압축 알고리즘 타입입니다. 기대치: {expected:#04x}, 실제: {found:#04x}")
            }
            MzcError::UnknownBlockType { found } => {
                write!(f, "알 수 없는 블록 타입입니다. 감지된 값: {found:#04x} (0x00=Literal, 0x01=Run, 0x02=Token, 0x03=BackRef)")
            }
            MzcError::TruncatedBlock { expected, found } => {
                write!(f, "블록 페이로드가 잘렸습니다. 기대 바이트 수: {expected}, 실제 바이트 수: {found}")
            }
            MzcError::OriginalSizeMismatch { expected, found } => {
                write!(f, "해제된 데이터의 크기가 원본 크기와 일치하지 않습니다. 기대치: {expected} bytes, 실제: {found} bytes")
            }
            MzcError::ChecksumMismatch { expected, found } => {
                write!(f, "데이터 무결성 검증 실패 (SHA-256 불일치)!\n  원본 해시: {expected}\n  복원 해시: {found}")
            }
            MzcError::CorruptDictionary => {
                write!(f, "MZC2 사전 데이터 섹션이 손상되었습니다. 바이트 오프셋 한계를 이탈했거나 카운트가 맞지 않습니다.")
            }
            MzcError::InvalidTokenIndex { index, max_valid } => {
                write!(f, "유효 범위를 벗어난 사전 토큰 인덱스 참조 발생! 참조 인덱스: {index}, 사전에 등록된 최대 인덱스 범위: {max_valid}")
            }
            MzcError::HuffmanError { message } => {
                write!(f, "허프만 엔트로피 디코딩 오류: {message}")
            }
            MzcError::InvalidBackRef { distance, length, current_size } => {
                write!(f, "유효 범위를 벗어난 LZ77 백레퍼런스 주소 참조 발생! 거리(Distance): {distance}, 길이(Length): {length}, 현재 복원된 데이터 크기: {current_size}")
            }
        }
    }
}

// MzcError가 Rust 표준 라이브러리의 Error 표준 트레이트를 동작하도록 설정합니다.
impl std::error::Error for MzcError {}
