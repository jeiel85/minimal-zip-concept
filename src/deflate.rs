use crate::error::MzcError;

// 이 파일은 업계 표준인 DEFLATE (RFC 1951) 및 GZIP (RFC 1952) 압축 스트림을 해제하는 역직렬화 디코더 모듈입니다.
// Rust 언어가 낯선 분들도 자료구조와 제어 흐름을 쉽게 공부할 수 있도록 아주 상세한 주석을 함께 적어두었습니다.

/// 입력 바이트 슬라이스로부터 비트(Bit) 단위로 데이터를 정밀하게 읽어들이는 헬퍼 구조체입니다.
/// 컴퓨터는 보통 바이트(8비트) 단위로 데이터를 처리하지만, 압축 포맷은 1비트나 3비트 같은 미세한 단위의 데이터를 쓰기 때문에 이 도구가 필요합니다.
struct BitReader<'a> {
    // Rust 개념 설명:
    // - `&'a [u8]`: 라이프타임(수명) 매개변수 `'a`가 포함된 바이트 슬라이스 참조자입니다.
    //   이 참조자가 가리키는 실제 데이터가 이 BitReader 구조체보다 더 오랫동안 메모리에 살아있음을 보장하여,
    //   메모리가 도중에 해제되어 꼬이는 버그(Dangling Pointer)를 방지하는 Rust의 고유 안전 장치입니다.
    bytes: &'a [u8],

    // 현재까지 비트스트림에서 몇 번째 비트까지 읽었는지 기록하는 누적 카운터입니다.
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    /// 새로운 BitReader 인스턴스를 바이트 데이터를 담아 생성합니다.
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, bit_pos: 0 }
    }

    /// 비트스트림으로부터 지정된 `n`개의 비트를 읽어 `u32` 정수 형태로 반환합니다.
    /// 데이터가 부족하여 비트를 더 읽을 수 없는 경우에는 `None`을 반환합니다.
    fn read_bits(&mut self, n: usize) -> Option<u32> {
        let mut val = 0u32;
        for i in 0..n {
            // 현재 읽어야 할 비트가 속해 있는 바이트의 배열 인덱스(몫)와 바이트 내부 비트 오프셋(나머지)을 계산합니다.
            let byte_idx = self.bit_pos / 8;
            let bit_idx = self.bit_pos % 8;

            // 데이터 경계를 초과해서 읽으려고 시도하면 None을 리턴하여 안전하게 에러 처리를 하도록 합니다.
            if byte_idx >= self.bytes.len() {
                return None;
            }

            // Rust 개념 설명:
            // - `&` 비트 연산자: 특정 자리에 비트가 켜져 있는지 확인합니다.
            // - `(1 << bit_idx)`: 1을 왼쪽으로 bit_idx만큼 밀어 켜고자 하는 비트 마스크를 만듭니다.
            let bit = (self.bytes[byte_idx] & (1 << bit_idx)) != 0;

            if bit {
                // 비트가 1이면, 반환값인 val의 i번째 비트 자리를 켭니다. (LE 방식으로 저장됨)
                val |= 1 << i;
            }
            // 비트를 한 개 읽었으므로 포인터를 1 증가시킵니다.
            self.bit_pos += 1;
        }
        Some(val)
    }

    /// 단 1비트만 읽어 참(true, 1) 혹은 거짓(false, 0) 값으로 변환하여 반환합니다.
    fn read_bit(&mut self) -> Option<bool> {
        // Rust 개념 설명:
        // - `.map(...)`: Option 타입이 Some(값)을 지니고 있으면 그 내부 값을 변형하고, None이면 그대로 None을 내보냅니다.
        //   1비트 정수 값을 bool로 직관적으로 매핑하기 위해 사용합니다.
        self.read_bits(1).map(|v| v != 0)
    }

    /// 비트 읽기 포인터를 다음 바이트 경계(8의 배수 비트 위치)로 맞춥니다.
    /// 비압축 데이터를 바이트 단위로 직접 읽기 전에 잔여 비트를 버리기 위해 호출합니다.
    fn align_byte(&mut self) {
        let remainder = self.bit_pos % 8;
        if remainder > 0 {
            self.bit_pos += 8 - remainder;
        }
    }
}

/// 허프만 트리(Huffman Tree)를 메모리상에 노드 기반으로 표현하기 위한 열거형(Enum)입니다.
/// 허프만 코딩은 자주 나오는 심볼에 짧은 비트를 부여하여 트리 형태로 검색하게 만듭니다.
#[derive(Clone, Copy, Debug)]
enum Node {
    // 내부 노드: 자식 노드로 연결되는 인덱스 정보(left, right)를 가집니다.
    Internal { left: u16, right: u16 },
    // 단말 노드: 실제 복원해 낼 16비트 심볼(단어) 값을 지니고 있습니다.
    Leaf { symbol: u16 },
}

/// RFC 1951 표준에 명시된 코드 길이(Code Length) 배열을 기준으로 허프만 트리 구조를 동적으로 빌드합니다.
///
/// # 매개변수:
/// - `lengths`: 각 기호(Symbol)들의 비트 길이를 순서대로 담은 슬라이스
///
/// # 반환값:
/// - 성공 시 빌드된 Node 벡터(트리)의 Some형, 불가능한 트리의 경우 None을 반환합니다.
fn build_huffman_tree(lengths: &[u8]) -> Option<Vec<Node>> {
    // 1. 트리 빌드를 위해 기호들의 코드 길이 중 최대값을 탐색합니다.
    let mut max_len = 0;
    for &len in lengths {
        if len > max_len {
            max_len = len;
        }
    }
    // 최대 길이가 0이면 빈 트리로 노드 하나만 구성해 즉시 리턴합니다.
    if max_len == 0 {
        return Some(vec![Node::Internal { left: 0, right: 0 }]);
    }

    // 2. 각 비트 길이별로 기호가 몇 개씩 들어있는지 카운팅합니다. (bl_count)
    let mut bl_count = vec![0u32; (max_len + 1) as usize];
    for &len in lengths {
        if len > 0 {
            bl_count[len as usize] += 1;
        }
    }

    // 3. 표준 스펙에 정의된 규칙대로 각 길이의 시작이 될 코드(Code) 값의 시작점을 찾아 배열에 저장합니다.
    let mut code = 0u32;
    let mut next_code = vec![0u32; (max_len + 1) as usize];
    for len in 1..=max_len as usize {
        code = (code + bl_count[len - 1]) << 1;
        next_code[len] = code;
    }

    // 4. 루트 노드(Internal, 0번) 하나로 시작하여 트리를 점진적으로 확장합니다.
    let mut nodes = vec![Node::Internal { left: 0, right: 0 }];

    for (sym, &len) in lengths.iter().enumerate() {
        if len == 0 {
            continue; // 비트 길이가 0인 기호는 이 파일에서 사용하지 않는 기호이므로 건너뜁니다.
        }
        // 기호의 비트 값과 순서쌍을 배정받습니다.
        let cur_code = next_code[len as usize];
        next_code[len as usize] += 1;

        // 배정받은 코드 비트스트림을 따라가며 트리에 가지(Branch)를 심어 나갑니다.
        let mut node_idx = 0;
        for bit_idx in (0..len).rev() {
            // 코드의 상위 비트부터 차례로 0인지 1인지 검사합니다.
            let bit = ((cur_code >> bit_idx) & 1) != 0;

            // 현재 탐색 중인 노드가 내부 노드(Internal)인지 매칭 구조로 확인하고 자식들을 추출합니다.
            let (left, right) = match nodes[node_idx] {
                Node::Internal { left, right } => (left, right),
                _ => return None, // 만약 잎(Leaf) 노드인데 자식을 심으려고 하면 손상된 상태이므로 실패(None) 처리합니다.
            };

            if !bit {
                // 비트가 0이면 왼쪽(left) 자식 노드 방향으로 탐색합니다.
                if left == 0 {
                    // 왼쪽 자식이 아직 비어 있다면 새 내부 노드를 할당해 연결합니다.
                    let next_idx = nodes.len() as u16;
                    nodes.push(Node::Internal { left: 0, right: 0 });
                    nodes[node_idx] = Node::Internal {
                        left: next_idx,
                        right,
                    };
                    node_idx = next_idx as usize;
                } else {
                    node_idx = left as usize;
                }
            } else {
                // 비트가 1이면 오른쪽(right) 자식 노드 방향으로 탐색합니다.
                if right == 0 {
                    let next_idx = nodes.len() as u16;
                    nodes.push(Node::Internal { left: 0, right: 0 });
                    nodes[node_idx] = Node::Internal {
                        left,
                        right: next_idx,
                    };
                    node_idx = next_idx as usize;
                } else {
                    node_idx = right as usize;
                }
            }
        }
        // 경로 끝까지 가지를 뻗은 마지막 노드 위치에 최종 단말 잎(Leaf)과 매핑될 실제 심볼을 할당합니다.
        nodes[node_idx] = Node::Leaf { symbol: sym as u16 };
    }
    Some(nodes)
}

/// BitReader로부터 비트를 하나씩 계속 읽으며 허프만 트리를 타고 내려가, 매칭되는 심볼을 1개 찾아서 복원합니다.
fn decode_symbol(reader: &mut BitReader, nodes: &[Node]) -> Option<u16> {
    let mut node_idx = 0;
    loop {
        // Rust 개념 설명:
        // - `match`: 다른 언어의 switch-case와 유사하지만 패턴 매칭을 통해 구조체의 값까지 안전하게 발라내는 아주 편리하고 안전한 문법입니다.
        match nodes[node_idx] {
            Node::Leaf { symbol } => return Some(symbol), // 단말 노드 도달 시 매칭된 기호 즉시 반환
            Node::Internal { left, right } => {
                // 내부 노드이면 비트를 하나 읽어 자식 중 어디로 이동할지 정합니다.
                let bit = reader.read_bit()?;
                let next = if !bit { left } else { right };
                if next == 0 {
                    return None; // 유효하지 않은 허프만 트리 경로 (파손 파일 지시)
                }
                node_idx = next as usize;
            }
        }
    }
}

/// RFC 1951 고정(Fixed) 허프만 모드에서 쓰이는 문자/길이 트리와 거리 트리를 코드값 기준으로 하드코딩 빌드합니다.
fn build_fixed_trees() -> (Vec<Node>, Vec<Node>) {
    // 스펙 상의 고정 길이 범위들 설정
    let mut lit_lengths = vec![0u8; 288];
    for i in 0..=143 {
        lit_lengths[i] = 8;
    }
    for i in 144..=255 {
        lit_lengths[i] = 9;
    }
    for i in 256..=279 {
        lit_lengths[i] = 7;
    }
    for i in 280..=287 {
        lit_lengths[i] = 8;
    }

    let mut dist_lengths = vec![0u8; 32];
    for i in 0..32 {
        dist_lengths[i] = 5;
    }

    (
        build_huffman_tree(&lit_lengths).unwrap(),
        build_huffman_tree(&dist_lengths).unwrap(),
    )
}

// DEFLATE 알고리즘의 길이(Length)를 복원하기 위한 기본값 표(Base)와 이에 더해 읽어야 할 추가 비트수 표(Extra)입니다.
const LENGTH_BASE: [u32; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

// DEFLATE 알고리즘의 백레퍼런스 복원용 거리(Distance) 기본값 표와 추가 비트수 표입니다.
const DISTANCE_BASE: [u32; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// raw DEFLATE 압축 이진 데이터를 입력받아 완전히 무손실 압축 해제(Decompress)하여 복원 데이터 바이트 배열을 반환합니다.
pub fn inflate(data: &[u8]) -> Result<Vec<u8>, MzcError> {
    let mut reader = BitReader::new(data);
    let mut out = Vec::new();

    // 고정 허프만 트리 테이블을 미리 구성합니다.
    let (fixed_lit_tree, fixed_dist_tree) = build_fixed_trees();

    loop {
        // 블록 종료 여부(BFINAL) 및 블록 형태(BTYPE) 정보를 리딩합니다.
        let bfinal = reader.read_bit().ok_or(MzcError::CorruptDictionary)?;
        let btype = reader.read_bits(2).ok_or(MzcError::CorruptDictionary)?;

        match btype {
            0 => {
                // 비압축(Uncompressed) 데이터 블록 처리
                reader.align_byte(); // 바이트 정렬
                let len = reader.read_bits(16).ok_or(MzcError::CorruptDictionary)? as usize;
                let _nlen = reader.read_bits(16).ok_or(MzcError::CorruptDictionary)?; // 1의 보수 검증 값

                for _ in 0..len {
                    let b = reader.read_bits(8).ok_or(MzcError::CorruptDictionary)? as u8;
                    out.push(b);
                }
            }
            1 | 2 => {
                // 1 = 고정 허프만 코더, 2 = 동적 허프만 코더
                let dynamic_trees = if btype == 2 {
                    // 동적 허프만 트리를 해독하기 위한 헤더 값을 파싱합니다.
                    let hlit = reader.read_bits(5).ok_or(MzcError::CorruptDictionary)? + 257;
                    let hdist = reader.read_bits(5).ok_or(MzcError::CorruptDictionary)? + 1;
                    let hclen = reader.read_bits(4).ok_or(MzcError::CorruptDictionary)? + 4;

                    // 코드 길이 복원용 코드 길이 테이블을 지정 순서쌍 순서대로 채워나갑니다.
                    let mut cl_lengths = vec![0u8; 19];
                    let cl_order = [
                        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                    ];
                    for i in 0..hclen as usize {
                        cl_lengths[cl_order[i]] =
                            reader.read_bits(3).ok_or(MzcError::CorruptDictionary)? as u8;
                    }

                    // 코드 길이를 해독할 트리 (Code Length Tree)를 구축합니다.
                    let cl_tree =
                        build_huffman_tree(&cl_lengths).ok_or(MzcError::CorruptDictionary)?;

                    let mut all_lengths = Vec::with_capacity((hlit + hdist) as usize);
                    while all_lengths.len() < (hlit + hdist) as usize {
                        let sym = decode_symbol(&mut reader, &cl_tree)
                            .ok_or(MzcError::CorruptDictionary)?;
                        if sym < 16 {
                            all_lengths.push(sym as u8);
                        } else if sym == 16 {
                            // 직전 길이를 3~6회 반복
                            let repeat =
                                reader.read_bits(2).ok_or(MzcError::CorruptDictionary)? + 3;
                            let last = *all_lengths.last().unwrap_or(&0);
                            for _ in 0..repeat {
                                all_lengths.push(last);
                            }
                        } else if sym == 17 {
                            // 0의 길이를 3~10회 반복
                            let repeat =
                                reader.read_bits(3).ok_or(MzcError::CorruptDictionary)? + 3;
                            for _ in 0..repeat {
                                all_lengths.push(0);
                            }
                        } else if sym == 18 {
                            // 0의 길이를 11~138회 반복
                            let repeat =
                                reader.read_bits(7).ok_or(MzcError::CorruptDictionary)? + 11;
                            for _ in 0..repeat {
                                all_lengths.push(0);
                            }
                        }
                    }

                    let lit_lengths = &all_lengths[0..hlit as usize];
                    let dist_lengths = &all_lengths[hlit as usize..];

                    // 완성된 코드 길이 데이터를 토대로 최종 인코딩 데이터 해석에 쓸 문장/길이 트리와 거리 트리를 빌드합니다.
                    let lt = build_huffman_tree(lit_lengths).ok_or(MzcError::CorruptDictionary)?;
                    let dt = build_huffman_tree(dist_lengths).ok_or(MzcError::CorruptDictionary)?;
                    Some((lt, dt))
                } else {
                    None
                };

                let actual_lit_tree = if btype == 1 {
                    &fixed_lit_tree
                } else if let Some((ref lt, _)) = dynamic_trees {
                    lt
                } else {
                    return Err(MzcError::CorruptDictionary);
                };

                let actual_dist_tree = if btype == 1 {
                    &fixed_dist_tree
                } else if let Some((_, ref dt)) = dynamic_trees {
                    dt
                } else {
                    return Err(MzcError::CorruptDictionary);
                };

                // 블록 내부 데이터를 심볼 단위로 순차 디코딩합니다.
                loop {
                    let sym = decode_symbol(&mut reader, actual_lit_tree)
                        .ok_or(MzcError::CorruptDictionary)?;
                    if sym < 256 {
                        // 리터럴 1바이트를 복원해 결과 버퍼에 씁니다.
                        out.push(sym as u8);
                    } else if sym == 256 {
                        // 블록 종료 기호 (End of Block)
                        break;
                    } else {
                        // 257 이상이면 LZ77 백레퍼런스(일치 단어 복사) 지시입니다.
                        let code_idx = (sym - 257) as usize;
                        if code_idx >= LENGTH_BASE.len() {
                            return Err(MzcError::CorruptDictionary);
                        }
                        // 기호 번호에 맞춰 일치 길이(Length)를 표에서 찾아 가져옵니다.
                        let base = LENGTH_BASE[code_idx];
                        let extra = LENGTH_EXTRA[code_idx];
                        let len = base
                            + reader
                                .read_bits(extra as usize)
                                .ok_or(MzcError::CorruptDictionary)?;

                        // 뒤이어 거리(Distance) 기호를 디코딩하고 거리를 계산합니다.
                        let dist_code = decode_symbol(&mut reader, actual_dist_tree)
                            .ok_or(MzcError::CorruptDictionary)?
                            as usize;
                        if dist_code >= DISTANCE_BASE.len() {
                            return Err(MzcError::CorruptDictionary);
                        }
                        let dist_base = DISTANCE_BASE[dist_code];
                        let dist_extra = DISTANCE_EXTRA[dist_code];
                        let dist = dist_base
                            + reader
                                .read_bits(dist_extra as usize)
                                .ok_or(MzcError::CorruptDictionary)?;

                        // 결과 버퍼 뒤편의 복사 위치를 특정하고 메모리에 저장되어 있던 이전 글자들을 반복해서 복사(Copy)해 옵니다.
                        let start_idx = out
                            .len()
                            .checked_sub(dist as usize)
                            .ok_or(MzcError::CorruptDictionary)?;
                        for offset in 0..len as usize {
                            if start_idx + offset >= out.len() {
                                return Err(MzcError::CorruptDictionary); // 메모리 오염/잘못된 거리 포인터 접근 방어
                            }
                            let val = out[start_idx + offset];
                            out.push(val);
                        }
                    }
                }
            }
            _ => return Err(MzcError::CorruptDictionary), // 허용되지 않는 BTYPE 3
        }

        // 마지막 블록(bfinal == true)이면 루프를 정지합니다.
        if bfinal {
            break;
        }
    }

    Ok(out)
}

/// 표준 GZIP 포맷 헤더(`.gz` 파일)의 필드 규격을 읽고 유효성을 완벽히 검증한 뒤,
/// 그 내부 알맹이인 DEFLATE 스트림을 해제하여 원본 데이터를 복원합니다.
pub fn gzip_decompress(bytes: &[u8]) -> Result<Vec<u8>, MzcError> {
    // GZIP 헤더 최소 10바이트 + 디플레이트 압축 바이트 + 푸터 최소 8바이트 = 총 18바이트 이상인지 검사합니다.
    if bytes.len() < 18 {
        return Err(MzcError::TruncatedHeader {
            read_bytes: bytes.len(),
        });
    }

    // GZIP의 매직 넘버 `0x1F`, `0x8B`를 검사하여 올바른 GZIP 파일인지 검증합니다.
    if bytes[0] != 0x1F || bytes[1] != 0x8B {
        return Err(MzcError::InvalidMagic {
            expected: "1F 8B".to_string(),
            found: format!("{:02X} {:02X}", bytes[0], bytes[1]),
        });
    }

    // 3번째 바이트인 압축 메커니즘(Compression Method) 값이 8(DEFLATE)인지 확인합니다.
    let cm = bytes[2];
    if cm != 8 {
        return Err(MzcError::InvalidAlgorithm {
            expected: 8,
            found: cm,
        });
    }

    // 4번째 바이트 플래그(FLG)를 추출하여 추가 필드의 탑재 유무를 확인합니다.
    let flg = bytes[3];

    let mut pos = 10; // 기본 헤더 크기 10바이트에서 시작

    // FEXTRA 플래그가 켜져 있으면 추가 헤더 영역의 크기만큼 포인터를 스킵합니다.
    if (flg & 0x04) != 0 {
        if pos + 2 > bytes.len() {
            return Err(MzcError::CorruptDictionary);
        }
        let xlen = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        pos += 2 + xlen;
    }

    // FNAME 플래그가 있으면 Null 바이트(0)가 나올 때까지 파일 이름을 건너뜁니다.
    if (flg & 0x08) != 0 {
        while pos < bytes.len() && bytes[pos] != 0 {
            pos += 1;
        }
        pos += 1; // Null 바이트 영역 통과
    }

    // FCOMMENT 플래그가 켜져 있으면 해독 주석 글자들을 동일하게 스킵합니다.
    if (flg & 0x10) != 0 {
        while pos < bytes.len() && bytes[pos] != 0 {
            pos += 1;
        }
        pos += 1;
    }

    // FHCRC 플래그가 있으면 헤더 자체 체크섬 2바이트를 지나갑니다.
    if (flg & 0x02) != 0 {
        pos += 2;
    }

    if pos >= bytes.len() {
        return Err(MzcError::CorruptDictionary);
    }

    // GZIP의 마지막 8바이트는 푸터(CRC32 체크섬과 원래 파일 크기 정보)이므로 제외하고,
    // 헤더 끝점(`pos`)부터 푸터 전까지의 구간을 순수 DEFLATE 페이로드로 보고 디코딩을 개시합니다.
    let deflate_data = &bytes[pos..bytes.len() - 8];
    let decompressed = inflate(deflate_data)?;

    // 푸터의 마지막 4바이트에서 기록되어 있는 원본 크기(Little-Endian u32)를 로드하여 복원된 크기와 대조 검사합니다.
    let footer_pos = bytes.len() - 8;
    let isize = u32::from_le_bytes([
        bytes[footer_pos + 4],
        bytes[footer_pos + 5],
        bytes[footer_pos + 6],
        bytes[footer_pos + 7],
    ]) as usize;

    if (decompressed.len() as u32) != (isize as u32) {
        return Err(MzcError::OriginalSizeMismatch {
            expected: isize as u64,
            found: decompressed.len() as u64,
        });
    }

    Ok(decompressed)
}
