use crate::error::MzcError;

/// MZC 파일의 고정 크기 헤더를 표현하는 구조체입니다.
/// MZC1(54바이트) 및 MZC2(56바이트) 스펙을 동시 수용하며 하위 호환성을 제공합니다.
///
/// # MZC2 헤더 명세 (56 Bytes):
/// 1. Magic Header       4 bytes   "MZC2" (MZC1의 경우 "MZC1")
/// 2. Version            1 byte    0x02   (MZC1의 경우 0x01)
/// 3. Algorithm Type     1 byte    0x01 = RLE / 0x02 = Dict-Only / 0x03 = Hybrid
/// 4. Original Size      8 bytes   u64 (Little-Endian)
/// 5. Payload Size       8 bytes   u64 (Little-Endian)
/// 6. Dictionary Size    2 bytes   u16 (Little-Endian, MZC1의 경우 부재하므로 0 세팅)
/// 7. Original SHA-256   32 bytes  (원본 데이터 해시)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MzcHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub algorithm_type: u8,
    pub original_size: u64,
    pub payload_size: u64,
    pub dictionary_size: u16,
    pub original_sha256: [u8; 32],
}

pub const MAGIC_MZC1: &[u8; 4] = b"MZC1";
pub const MAGIC_MZC2: &[u8; 4] = b"MZC2";
pub const MAGIC_MZC3: &[u8; 4] = b"MZC3";
pub const MAGIC_MZC4: &[u8; 4] = b"MZC4";
pub const MAGIC_MZC5: &[u8; 4] = b"MZC5";

pub const VERSION_MZC1: u8 = 0x01;
pub const VERSION_MZC2: u8 = 0x02;
pub const VERSION_MZC3: u8 = 0x03;
pub const VERSION_MZC4: u8 = 0x04;
pub const VERSION_MZC5: u8 = 0x05;

pub const ALGORITHM_RLE: u8 = 0x01;
pub const ALGORITHM_DICT: u8 = 0x02;
pub const ALGORITHM_HYBRID: u8 = 0x03;
pub const ALGORITHM_LZ77: u8 = 0x04;

pub const FILTER_DELTA: u8 = 0x10;
pub const FILTER_BCJ: u8 = 0x20;
pub const FILTER_DYNAMIC_HUFFMAN: u8 = 0x40;

pub const HEADER_SIZE_MZC1: usize = 54;
pub const HEADER_SIZE_MZC2: usize = 56;
pub const HEADER_SIZE_MZC3: usize = 56;
pub const HEADER_SIZE_MZC4: usize = 56;
pub const HEADER_SIZE_MZC5: usize = 56;

impl MzcHeader {
    /// MZC5(버전 5) 기반의 새 헤더 구조체를 생성합니다.
    pub fn new_v5(
        algorithm_type: u8,
        original_size: u64,
        payload_size: u64,
        dictionary_size: u16,
        sha256: [u8; 32],
    ) -> Self {
        Self {
            magic: *MAGIC_MZC5,
            version: VERSION_MZC5,
            algorithm_type,
            original_size,
            payload_size,
            dictionary_size,
            original_sha256: sha256,
        }
    }

    /// MZC4(버전 4) 기반의 새 헤더 구조체를 생성합니다.
    pub fn new_v4(
        algorithm_type: u8,
        original_size: u64,
        payload_size: u64,
        dictionary_size: u16,
        sha256: [u8; 32],
    ) -> Self {
        Self {
            magic: *MAGIC_MZC4,
            version: VERSION_MZC4,
            algorithm_type,
            original_size,
            payload_size,
            dictionary_size,
            original_sha256: sha256,
        }
    }

    /// MZC3(버전 3) 기반의 새 헤더 구조체를 생성합니다.
    pub fn new_v3(
        algorithm_type: u8,
        original_size: u64,
        payload_size: u64,
        dictionary_size: u16,
        sha256: [u8; 32],
    ) -> Self {
        Self {
            magic: *MAGIC_MZC3,
            version: VERSION_MZC3,
            algorithm_type,
            original_size,
            payload_size,
            dictionary_size,
            original_sha256: sha256,
        }
    }

    /// MZC2(버전 2) 기반의 새 헤더 구조체를 생성합니다.
    pub fn new_v2(
        algorithm_type: u8,
        original_size: u64,
        payload_size: u64,
        dictionary_size: u16,
        sha256: [u8; 32],
    ) -> Self {
        Self {
            magic: *MAGIC_MZC2,
            version: VERSION_MZC2,
            algorithm_type,
            original_size,
            payload_size,
            dictionary_size,
            original_sha256: sha256,
        }
    }

    /// MZC1(버전 1) 기반의 새 헤더 구조체를 생성합니다.
    pub fn new_v1(original_size: u64, payload_size: u64, sha256: [u8; 32]) -> Self {
        Self {
            magic: *MAGIC_MZC1,
            version: VERSION_MZC1,
            algorithm_type: ALGORITHM_RLE,
            original_size,
            payload_size,
            dictionary_size: 0,
            original_sha256: sha256,
        }
    }

    /// 현재 헤더 정보의 버전에 부합하는 고정 바이트 배열(`Vec<u8>`)로 직렬화합니다.
    pub fn to_bytes(&self) -> Vec<u8> {
        let size = if self.version == VERSION_MZC2 || self.version == VERSION_MZC3 || self.version == VERSION_MZC4 || self.version == VERSION_MZC5 {
            HEADER_SIZE_MZC2
        } else {
            HEADER_SIZE_MZC1
        };
        let mut bytes = Vec::with_capacity(size);

        // 1. Magic (4 bytes)
        bytes.extend_from_slice(&self.magic);

        // 2. Version (1 byte)
        bytes.push(self.version);

        // 3. Algorithm Type (1 byte)
        bytes.push(self.algorithm_type);

        // 4. Original Size (8 bytes, little-endian)
        bytes.extend_from_slice(&self.original_size.to_le_bytes());

        // 5. Payload Size (8 bytes, little-endian)
        bytes.extend_from_slice(&self.payload_size.to_le_bytes());

        // MZC2(버전 2), MZC3(버전 3), MZC4(버전 4) 및 MZC5(버전 5) 전용 필드 직렬화
        if self.version == VERSION_MZC2 || self.version == VERSION_MZC3 || self.version == VERSION_MZC4 || self.version == VERSION_MZC5 {
            // 6. Dictionary Size (2 bytes, little-endian)
            bytes.extend_from_slice(&self.dictionary_size.to_le_bytes());
        }

        // 7. Original SHA-256 (32 bytes)
        bytes.extend_from_slice(&self.original_sha256);

        bytes
    }

    /// 이진 데이터 슬라이스를 해독하여 MzcHeader를 복구합니다.
    /// 구버전 "MZC1", 신버전 "MZC2", 그리고 최신 "MZC3" 규격을 시작 매직 바이트로 판별하는 이중 파싱 분기(Double-Dispatch)를 수행합니다.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MzcError> {
        if bytes.len() < 4 {
            return Err(MzcError::TruncatedHeader { read_bytes: bytes.len() });
        }

        let magic: [u8; 4] = bytes[0..4]
            .try_into()
            .expect("시작 4바이트 슬라이스 변환은 반드시 통과해야 합니다.");

        if magic == *MAGIC_MZC1 {
            // ================== MZC1 (54바이트) 파싱 ==================
            if bytes.len() < HEADER_SIZE_MZC1 {
                return Err(MzcError::TruncatedHeader { read_bytes: bytes.len() });
            }

            let version = bytes[4];
            if version != VERSION_MZC1 {
                return Err(MzcError::InvalidVersion {
                    expected: VERSION_MZC1,
                    found: version,
                });
            }

            let algorithm_type = bytes[5];
            if algorithm_type != ALGORITHM_RLE {
                return Err(MzcError::InvalidAlgorithm {
                    expected: ALGORITHM_RLE,
                    found: algorithm_type,
                });
            }

            let original_size_bytes: [u8; 8] = bytes[6..14]
                .try_into()
                .expect("u64 파싱용 8바이트 슬라이스 획득");
            let original_size = u64::from_le_bytes(original_size_bytes);

            let payload_size_bytes: [u8; 8] = bytes[14..22]
                .try_into()
                .expect("u64 파싱용 8바이트 슬라이스 획득");
            let payload_size = u64::from_le_bytes(payload_size_bytes);

            let mut original_sha256 = [0u8; 32];
            original_sha256.copy_from_slice(&bytes[22..54]);

            Ok(Self {
                magic,
                version,
                algorithm_type,
                original_size,
                payload_size,
                dictionary_size: 0,
                original_sha256,
            })
        } else if magic == *MAGIC_MZC2 {
            // ================== MZC2 (56바이트) 파싱 ==================
            if bytes.len() < HEADER_SIZE_MZC2 {
                return Err(MzcError::TruncatedHeader { read_bytes: bytes.len() });
            }

            let version = bytes[4];
            if version != VERSION_MZC2 {
                return Err(MzcError::InvalidVersion {
                    expected: VERSION_MZC2,
                    found: version,
                });
            }

            let algorithm_type = bytes[5];
            if algorithm_type != ALGORITHM_RLE
                && algorithm_type != ALGORITHM_DICT
                && algorithm_type != ALGORITHM_HYBRID
            {
                return Err(MzcError::InvalidAlgorithm {
                    expected: ALGORITHM_HYBRID, // 기대 유형 중 하나 명시
                    found: algorithm_type,
                });
            }

            let original_size_bytes: [u8; 8] = bytes[6..14]
                .try_into()
                .expect("u64 파싱용 8바이트 슬라이스 획득");
            let original_size = u64::from_le_bytes(original_size_bytes);

            let payload_size_bytes: [u8; 8] = bytes[14..22]
                .try_into()
                .expect("u64 파싱용 8바이트 슬라이스 획득");
            let payload_size = u64::from_le_bytes(payload_size_bytes);

            // 22~24바이트 영역에서 사전 크기 u16 복구
            let dictionary_size_bytes: [u8; 2] = bytes[22..24]
                .try_into()
                .expect("u16 사전크기 파싱용 2바이트 슬라이스 획득");
            let dictionary_size = u16::from_le_bytes(dictionary_size_bytes);

            // 24~56바이트 영역에서 원본 SHA-256 복구
            let mut original_sha256 = [0u8; 32];
            original_sha256.copy_from_slice(&bytes[24..56]);

            Ok(Self {
                magic,
                version,
                algorithm_type,
                original_size,
                payload_size,
                dictionary_size,
                original_sha256,
            })
        } else if magic == *MAGIC_MZC3 {
            // ================== MZC3 (56바이트) 파싱 ==================
            if bytes.len() < HEADER_SIZE_MZC3 {
                return Err(MzcError::TruncatedHeader { read_bytes: bytes.len() });
            }

            let version = bytes[4];
            if version != VERSION_MZC3 {
                return Err(MzcError::InvalidVersion {
                    expected: VERSION_MZC3,
                    found: version,
                });
            }

            let algorithm_type = bytes[5];
            if algorithm_type != ALGORITHM_RLE
                && algorithm_type != ALGORITHM_DICT
                && algorithm_type != ALGORITHM_HYBRID
                && algorithm_type != ALGORITHM_LZ77
            {
                return Err(MzcError::InvalidAlgorithm {
                    expected: ALGORITHM_LZ77,
                    found: algorithm_type,
                });
            }

            let original_size_bytes: [u8; 8] = bytes[6..14]
                .try_into()
                .expect("u64 파싱용 8바이트 슬라이스 획득");
            let original_size = u64::from_le_bytes(original_size_bytes);

            let payload_size_bytes: [u8; 8] = bytes[14..22]
                .try_into()
                .expect("u64 파싱용 8바이트 슬라이스 획득");
            let payload_size = u64::from_le_bytes(payload_size_bytes);

            let dictionary_size_bytes: [u8; 2] = bytes[22..24]
                .try_into()
                .expect("u16 사전크기 파싱용 2바이트 슬라이스 획득");
            let dictionary_size = u16::from_le_bytes(dictionary_size_bytes);

            let mut original_sha256 = [0u8; 32];
            original_sha256.copy_from_slice(&bytes[24..56]);

            Ok(Self {
                magic,
                version,
                algorithm_type,
                original_size,
                payload_size,
                dictionary_size,
                original_sha256,
            })
        } else if magic == *MAGIC_MZC4 {
            // ================== MZC4 (56바이트) 파싱 ==================
            if bytes.len() < HEADER_SIZE_MZC2 {
                return Err(MzcError::TruncatedHeader { read_bytes: bytes.len() });
            }

            let version = bytes[4];
            if version != VERSION_MZC4 {
                return Err(MzcError::InvalidVersion {
                    expected: VERSION_MZC4,
                    found: version,
                });
            }

            let algorithm_type = bytes[5];
            if algorithm_type != ALGORITHM_RLE
                && algorithm_type != ALGORITHM_DICT
                && algorithm_type != ALGORITHM_HYBRID
                && algorithm_type != ALGORITHM_LZ77
            {
                return Err(MzcError::InvalidAlgorithm {
                    expected: ALGORITHM_LZ77,
                    found: algorithm_type,
                });
            }

            let original_size_bytes: [u8; 8] = bytes[6..14]
                .try_into()
                .expect("u64 파싱용 8바이트 슬라이스 획득");
            let original_size = u64::from_le_bytes(original_size_bytes);

            let payload_size_bytes: [u8; 8] = bytes[14..22]
                .try_into()
                .expect("u64 파싱용 8바이트 슬라이스 획득");
            let payload_size = u64::from_le_bytes(payload_size_bytes);

            let dictionary_size_bytes: [u8; 2] = bytes[22..24]
                .try_into()
                .expect("u16 사전크기 파싱용 2바이트 슬라이스 획득");
            let dictionary_size = u16::from_le_bytes(dictionary_size_bytes);

            let mut original_sha256 = [0u8; 32];
            original_sha256.copy_from_slice(&bytes[24..56]);

            Ok(Self {
                magic,
                version,
                algorithm_type,
                original_size,
                payload_size,
                dictionary_size,
                original_sha256,
            })
        } else if magic == *MAGIC_MZC5 {
            // ================== MZC5 (56바이트) 파싱 ==================
            if bytes.len() < HEADER_SIZE_MZC5 {
                return Err(MzcError::TruncatedHeader { read_bytes: bytes.len() });
            }

            let version = bytes[4];
            if version != VERSION_MZC5 {
                return Err(MzcError::InvalidVersion {
                    expected: VERSION_MZC5,
                    found: version,
                });
            }

            let algorithm_type = bytes[5];
            let core_alg = algorithm_type & 0x0F;
            if core_alg != ALGORITHM_RLE
                && core_alg != ALGORITHM_DICT
                && core_alg != ALGORITHM_HYBRID
                && core_alg != ALGORITHM_LZ77
            {
                return Err(MzcError::InvalidAlgorithm {
                    expected: ALGORITHM_LZ77,
                    found: core_alg,
                });
            }

            let original_size_bytes: [u8; 8] = bytes[6..14]
                .try_into()
                .expect("u64 파싱용 8바이트 슬라이스 획득");
            let original_size = u64::from_le_bytes(original_size_bytes);

            let payload_size_bytes: [u8; 8] = bytes[14..22]
                .try_into()
                .expect("u64 파싱용 8바이트 슬라이스 획득");
            let payload_size = u64::from_le_bytes(payload_size_bytes);

            let dictionary_size_bytes: [u8; 2] = bytes[22..24]
                .try_into()
                .expect("u16 사전크기 파싱용 2바이트 슬라이스 획득");
            let dictionary_size = u16::from_le_bytes(dictionary_size_bytes);

            let mut original_sha256 = [0u8; 32];
            original_sha256.copy_from_slice(&bytes[24..56]);

            Ok(Self {
                magic,
                version,
                algorithm_type,
                original_size,
                payload_size,
                dictionary_size,
                original_sha256,
            })
        } else {
            // 매직넘버 모두 틀린 규정 외 오염 파일
            let found = String::from_utf8_lossy(&magic).into_owned();
            let expected = "MZC1, MZC2, MZC3, MZC4 or MZC5".to_string();
            Err(MzcError::InvalidMagic { expected, found })
        }
    }
}
