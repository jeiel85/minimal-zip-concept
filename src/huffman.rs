use crate::error::MzcError;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// 허프만 트리 구축을 위한 노드 구조체입니다.
/// 최소 힙(Min-Heap)으로 작동할 수 있도록 `Ord`와 `PartialOrd` 트레이트를 사용자 정의합니다.
#[derive(Debug, Eq, PartialEq)]
struct HuffmanNode {
    symbol: Option<u8>,
    frequency: usize,
    left: Option<Box<HuffmanNode>>,
    right: Option<Box<HuffmanNode>>,
}

impl HuffmanNode {
    fn new_leaf(symbol: u8, frequency: usize) -> Self {
        Self {
            symbol: Some(symbol),
            frequency,
            left: None,
            right: None,
        }
    }

    fn new_internal(frequency: usize, left: HuffmanNode, right: HuffmanNode) -> Self {
        Self {
            symbol: None,
            frequency,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }
}

// 빈도수가 더 작은 노드가 우선순위 큐(BinaryHeap)에서 높은 우선순위를 갖도록(Min-heap) 비교 기준을 역순으로 정의합니다.
impl Ord for HuffmanNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.frequency.cmp(&self.frequency)
    }
}

impl PartialOrd for HuffmanNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 비트스트림 데이터를 바이트 벡터에 비트 단위로 안전하게 써나가는 구조체입니다.
#[derive(Debug, Default)]
pub struct BitWriter {
    pub bytes: Vec<u8>,
    current_byte: u8,
    bit_count: u8,
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            current_byte: 0,
            bit_count: 0,
        }
    }

    /// 1비트를 씁니다. true 이면 1, false 이면 0을 씁니다.
    pub fn write_bit(&mut self, bit: bool) {
        if bit {
            // current_byte의 현재 비트 위치에 1을 채웁니다. MSB부터 씁니다.
            self.current_byte |= 1 << (7 - self.bit_count);
        }
        self.bit_count += 1;

        // 8비트가 꽉 차면 바이트 벡터에 밀어 넣고 초기화합니다.
        if self.bit_count == 8 {
            self.bytes.push(self.current_byte);
            self.current_byte = 0;
            self.bit_count = 0;
        }
    }

    /// 여러 비트를 하위 비트부터 씁니다.
    pub fn write_bits(&mut self, value: u32, num_bits: usize) {
        for i in (0..num_bits).rev() {
            let bit = ((value >> i) & 1) == 1;
            self.write_bit(bit);
        }
    }

    /// 남은 비트가 있다면 패딩(0)을 채워서 최종 바이트를 출력 버퍼에 씁니다.
    pub fn flush(&mut self) {
        if self.bit_count > 0 {
            self.bytes.push(self.current_byte);
            self.current_byte = 0;
            self.bit_count = 0;
        }
    }
}

/// 바이트 슬라이스로부터 비트 단위로 안전하게 읽어오는 리더 구조체입니다.
#[derive(Debug)]
pub struct BitReader<'a> {
    bytes: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    /// 1비트를 읽어옵니다. 데이터가 종결되어 더 읽을 비트가 없으면 에러를 리턴합니다.
    pub fn read_bit(&mut self) -> Result<bool, MzcError> {
        if self.byte_pos >= self.bytes.len() {
            return Err(MzcError::HuffmanError {
                message: "비트스트림 읽기 도중 예기치 못하게 EOF를 만났습니다.".to_string(),
            });
        }

        let bit = ((self.bytes[self.byte_pos] >> (7 - self.bit_pos)) & 1) == 1;
        self.bit_pos += 1;

        if self.bit_pos == 8 {
            self.byte_pos += 1;
            self.bit_pos = 0;
        }

        Ok(bit)
    }
}

/// 정적 허프만 트리를 빌드하고 코드 테이블을 리턴하는 헬퍼 함수입니다.
/// 모든 기호의 빈도를 수집한 256 크기의 빈도표 배열(`[u32; 256]`)을 입력으로 받습니다.
fn build_huffman_tree(frequencies: &[u32; 256]) -> Option<HuffmanNode> {
    let mut heap = BinaryHeap::new();

    // 빈도수가 1 이상인 기호만 수집해 우선순위 큐(BinaryHeap)에 넣습니다.
    for symbol in 0..256 {
        let freq = frequencies[symbol] as usize;
        if freq > 0 {
            heap.push(HuffmanNode::new_leaf(symbol as u8, freq));
        }
    }

    // 파일 전체가 단 하나의 바이트 종류로만 이뤄졌거나 비어있는 특수 케이스 처리
    if heap.is_empty() {
        return None;
    }
    if heap.len() == 1 {
        // 더미용 노드를 하나 결합해 트리가 성립하게 해줍니다.
        let single_node = heap.pop().unwrap();
        let parent = HuffmanNode::new_internal(
            single_node.frequency,
            single_node,
            HuffmanNode::new_leaf(0, 0), // 더미 노드
        );
        return Some(parent);
    }

    // 두 개의 최소 노드를 계속 합쳐 하나의 트리로 완성합니다.
    while heap.len() > 1 {
        let left = heap.pop().unwrap();
        let right = heap.pop().unwrap();
        let parent = HuffmanNode::new_internal(left.frequency + right.frequency, left, right);
        heap.push(parent);
    }

    heap.pop()
}

/// 재귀적으로 트리를 탐색하며 각 기호에 상응하는 비트 코드 리스트를 매핑 테이블에 저장합니다.
fn generate_codes(
    node: &HuffmanNode,
    current_code: u32,
    current_depth: usize,
    table: &mut [(u32, usize); 256],
) {
    if let Some(symbol) = node.symbol {
        // 단말 노드에 도달했으므로 획득한 코드와 깊이를 기록합니다.
        table[symbol as usize] = (current_code, current_depth);
        return;
    }

    if let Some(ref left) = node.left {
        generate_codes(left, current_code << 1, current_depth + 1, table);
    }
    if let Some(ref right) = node.right {
        generate_codes(right, (current_code << 1) | 1, current_depth + 1, table);
    }
}

/// 입력 바이트 데이터를 정적 허프만 코딩 방식으로 압축하고, 직렬화된 빈도표 헤더(1024바이트)를 포함한 바이트 벡터를 리턴합니다.
pub fn huffman_compress(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        // 빈 데이터인 경우 1024바이트의 0 빈도표 헤더만 리턴합니다.
        return vec![0u8; 1024];
    }

    // 1. 바이트별 빈도수 조사 (256바이트 빈도표 작성)
    let mut frequencies = [0u32; 256];
    for &byte in data {
        frequencies[byte as usize] += 1;
    }

    // 2. 허프만 트리 빌드
    let tree = build_huffman_tree(&frequencies)
        .expect("1개 이상의 기호가 감지되어 트리가 성립되어야 합니다.");

    // 3. 코드 테이블 맵 생성
    let mut code_table = [(0u32, 0usize); 256];
    generate_codes(&tree, 0, 0, &mut code_table);

    // 4. 출력 바이트 패킹 준비
    // MZC2의 1024바이트 고정 빈도표 직렬화 (각 u32를 Little-Endian 4바이트로 변환)
    let mut output = Vec::with_capacity(1024 + (data.len() / 2));
    for symbol in 0..256 {
        output.extend_from_slice(&frequencies[symbol].to_le_bytes());
    }

    // 5. 비트 단위 인코딩
    let mut bit_writer = BitWriter::new();
    for &byte in data {
        let (code, num_bits) = code_table[byte as usize];
        bit_writer.write_bits(code, num_bits);
    }
    bit_writer.flush();

    output.extend_from_slice(&bit_writer.bytes);
    output
}

/// 직렬화된 1024바이트 고정 빈도표 헤더를 파싱하고 뒤이은 비트스트림 데이터를 원래 바이트 배열로 복구합니다.
///
/// # 파라미터:
/// - `compressed_payload`: [1024바이트 빈도표 헤더] + [가변 비트스트림] 구조
/// - `original_size`: 해제 대상의 기대 원본 크기
pub fn huffman_decompress(
    compressed_payload: &[u8],
    original_size: usize,
) -> Result<Vec<u8>, MzcError> {
    if original_size == 0 {
        return Ok(Vec::new());
    }

    if compressed_payload.len() < 1024 {
        return Err(MzcError::HuffmanError {
            message: "허프만 페이로드 데이터가 너무 짧습니다. 최소 1024바이트 빈도표가 필요합니다."
                .to_string(),
        });
    }

    // 1. 1024바이트 고정 빈도표 파싱
    let mut frequencies = [0u32; 256];
    for symbol in 0..256 {
        let offset = symbol * 4;
        let bytes: [u8; 4] = compressed_payload[offset..offset + 4]
            .try_into()
            .map_err(|_| MzcError::HuffmanError {
                message: "빈도표 u32 파싱 슬라이스 획득에 실패했습니다.".to_string(),
            })?;
        frequencies[symbol] = u32::from_le_bytes(bytes);
    }

    // 2. 허프만 트리 재구축
    let tree = build_huffman_tree(&frequencies).ok_or_else(|| MzcError::HuffmanError {
        message: "빈도표가 비어있어 허프만 트리를 재구축할 수 없습니다.".to_string(),
    })?;

    // 3. 비트스트림을 순차 탐색하며 바이트 복원 수행
    let bitstream_bytes = &compressed_payload[1024..];
    let mut bit_reader = BitReader::new(bitstream_bytes);
    let mut decompressed = Vec::with_capacity(original_size);

    while decompressed.len() < original_size {
        let mut current_node = &tree;

        // 단말 노드(Leaf Node)에 도달할 때까지 트리 탐색을 수행합니다.
        while current_node.symbol.is_none() {
            let bit = bit_reader.read_bit()?;
            if bit {
                // 1이면 우측 자식 노드로 이동
                current_node =
                    current_node
                        .right
                        .as_ref()
                        .ok_or_else(|| MzcError::HuffmanError {
                            message: "허프만 트리 우측 리프 분기가 손상되었습니다.".to_string(),
                        })?;
            } else {
                // 0이면 좌측 자식 노드로 이동
                current_node =
                    current_node
                        .left
                        .as_ref()
                        .ok_or_else(|| MzcError::HuffmanError {
                            message: "허프만 트리 좌측 리프 분기가 손상되었습니다.".to_string(),
                        })?;
            }
        }

        // 단말 노드의 기호를 출력 버퍼에 덧붙입니다.
        if let Some(symbol) = current_node.symbol {
            decompressed.push(symbol);
        }
    }

    Ok(decompressed)
}

fn get_code_lengths(node: &HuffmanNode, current_depth: usize, lengths: &mut [u8; 256]) {
    if let Some(symbol) = node.symbol {
        lengths[symbol as usize] = current_depth as u8;
        return;
    }
    if let Some(ref left) = node.left {
        get_code_lengths(left, current_depth + 1, lengths);
    }
    if let Some(ref right) = node.right {
        get_code_lengths(right, current_depth + 1, lengths);
    }
}

pub fn build_canonical_codes(lengths: &[u8; 256]) -> [(u32, usize); 256] {
    let mut code_table = [(0u32, 0usize); 256];
    let mut len_counts = [0u32; 33];
    for &len in lengths {
        if len > 0 {
            len_counts[len as usize] += 1;
        }
    }
    let mut next_code = [0u32; 33];
    let mut code = 0u32;
    for len in 1..=32 {
        code = (code + len_counts[len - 1]) << 1;
        next_code[len] = code;
    }
    for symbol in 0..256 {
        let len = lengths[symbol] as usize;
        if len > 0 {
            code_table[symbol] = (next_code[len], len);
            next_code[len] += 1;
        }
    }
    code_table
}

pub fn compress_code_lengths(lengths: &[u8; 256]) -> Vec<u8> {
    let mut compressed = Vec::new();
    let mut i = 0;
    while i < 256 {
        let val = lengths[i];
        if val == 0 {
            let mut run_len = 0;
            while i + run_len < 256 && lengths[i + run_len] == 0 && run_len < 128 {
                run_len += 1;
            }
            compressed.push(0x80 | ((run_len - 1) as u8));
            i += run_len;
        } else {
            let mut run_len = 0;
            while i + run_len < 256 && lengths[i + run_len] == val && run_len < 4 {
                run_len += 1;
            }
            let l_val = std::cmp::min(val, 31);
            compressed.push((l_val << 2) | ((run_len - 1) as u8));
            i += run_len;
        }
    }
    compressed
}

pub fn decompress_code_lengths(compressed: &[u8]) -> Result<[u8; 256], MzcError> {
    let mut lengths = [0u8; 256];
    let mut idx = 0;
    for &byte in compressed {
        if idx >= 256 {
            break;
        }
        if (byte & 0x80) != 0 {
            let run_len = ((byte & 0x7F) as usize) + 1;
            if idx + run_len > 256 {
                return Err(MzcError::HuffmanError {
                    message: "오버플로우: 디코딩된 코드 길이 배열의 크기가 256을 초과합니다."
                        .to_string(),
                });
            }
            for i in 0..run_len {
                lengths[idx + i] = 0;
            }
            idx += run_len;
        } else {
            let l_val = (byte >> 2) & 0x1F;
            let run_len = ((byte & 0x03) as usize) + 1;
            if idx + run_len > 256 {
                return Err(MzcError::HuffmanError {
                    message: "오버플로우: 디코딩된 코드 길이 배열의 크기가 256을 초과합니다."
                        .to_string(),
                });
            }
            for i in 0..run_len {
                lengths[idx + i] = l_val;
            }
            idx += run_len;
        }
    }
    if idx < 256 {
        return Err(MzcError::HuffmanError {
            message: format!(
                "과소 디코딩: 코드 길이가 256개 채워지지 않았습니다 (채워진 개수: {}).",
                idx
            ),
        });
    }
    Ok(lengths)
}

pub fn huffman_compress_dynamic(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return vec![0u8, 0u8];
    }
    let mut frequencies = [0u32; 256];
    for &byte in data {
        frequencies[byte as usize] += 1;
    }
    let tree = build_huffman_tree(&frequencies)
        .expect("1개 이상의 기호가 감지되어 트리가 성립되어야 합니다.");
    let mut code_lengths = [0u8; 256];
    get_code_lengths(&tree, 0, &mut code_lengths);
    let code_table = build_canonical_codes(&code_lengths);
    let compressed_tree = compress_code_lengths(&code_lengths);
    let tree_len = compressed_tree.len() as u16;
    let mut output = Vec::with_capacity(2 + compressed_tree.len() + (data.len() / 2));
    output.extend_from_slice(&tree_len.to_le_bytes());
    output.extend_from_slice(&compressed_tree);
    let mut bit_writer = BitWriter::new();
    for &byte in data {
        let (code, num_bits) = code_table[byte as usize];
        bit_writer.write_bits(code, num_bits);
    }
    bit_writer.flush();
    output.extend_from_slice(&bit_writer.bytes);
    output
}

pub fn huffman_decompress_dynamic(
    compressed_payload: &[u8],
    original_size: usize,
) -> Result<Vec<u8>, MzcError> {
    if original_size == 0 {
        return Ok(Vec::new());
    }
    if compressed_payload.len() < 2 {
        return Err(MzcError::HuffmanError {
            message: "동적 허프만 페이로드 데이터가 너무 짧습니다.".to_string(),
        });
    }
    let tree_len_bytes: [u8; 2] = compressed_payload[0..2].try_into().unwrap();
    let tree_len = u16::from_le_bytes(tree_len_bytes) as usize;
    if compressed_payload.len() < 2 + tree_len {
        return Err(MzcError::HuffmanError {
            message: "동적 허프만 트리 헤더 영역이 손상되었습니다.".to_string(),
        });
    }
    let compressed_tree = &compressed_payload[2..2 + tree_len];
    let bitstream_bytes = &compressed_payload[2 + tree_len..];
    let code_lengths = decompress_code_lengths(compressed_tree)?;
    let codes = build_canonical_codes(&code_lengths);
    let mut tree = HuffmanNode {
        symbol: None,
        frequency: 0,
        left: None,
        right: None,
    };
    for symbol in 0..256 {
        let (code, len) = codes[symbol];
        if len > 0 {
            let mut current = &mut tree;
            for i in (0..len).rev() {
                let bit = ((code >> i) & 1) == 1;
                if bit {
                    if current.right.is_none() {
                        current.right = Some(Box::new(HuffmanNode {
                            symbol: None,
                            frequency: 0,
                            left: None,
                            right: None,
                        }));
                    }
                    current = current.right.as_mut().unwrap();
                } else {
                    if current.left.is_none() {
                        current.left = Some(Box::new(HuffmanNode {
                            symbol: None,
                            frequency: 0,
                            left: None,
                            right: None,
                        }));
                    }
                    current = current.left.as_mut().unwrap();
                }
            }
            current.symbol = Some(symbol as u8);
        }
    }
    let mut bit_reader = BitReader::new(bitstream_bytes);
    let mut decompressed = Vec::with_capacity(original_size);
    while decompressed.len() < original_size {
        let mut current_node = &tree;
        while current_node.symbol.is_none() {
            let bit = bit_reader.read_bit()?;
            if bit {
                current_node =
                    current_node
                        .right
                        .as_ref()
                        .ok_or_else(|| MzcError::HuffmanError {
                            message: "허프만 트리 우측 리프 분기가 손상되었습니다.".to_string(),
                        })?;
            } else {
                current_node =
                    current_node
                        .left
                        .as_ref()
                        .ok_or_else(|| MzcError::HuffmanError {
                            message: "허프만 트리 좌측 리프 분기가 손상되었습니다.".to_string(),
                        })?;
            }
        }
        if let Some(symbol) = current_node.symbol {
            decompressed.push(symbol);
        }
    }
    Ok(decompressed)
}
