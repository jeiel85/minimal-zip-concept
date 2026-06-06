use sha2::{Digest, Sha256};

pub fn calculate_crc32(data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

/// 주어진 바이트 슬라이스(&[u8])의 SHA-256 해시 값을 계산하여 32바이트 고정 크기 배열로 반환합니다.
///
/// # Rust 개념 설명:
/// - `&[u8]`: 바이트의 '슬라이스'를 나타내며, 메모리에 있는 임의의 바이트 데이터 시퀀스에 대한 참조입니다.
///   이 방식을 사용하면 복사(Copy)를 피하고 원본 데이터를 가리켜 성능을 최적화할 수 있습니다.
/// - `[u8; 32]`: 크기가 정확히 32바이트인 고정 크기 배열 타입입니다.
pub fn calculate_sha256(data: &[u8]) -> [u8; 32] {
    // Sha256 구조체의 새로운 인스턴스를 생성(초기화)합니다.
    let mut hasher = Sha256::new();

    // 데이터를 hasher에 입력합니다. 데이터 양이 많으면 여러 번 update()할 수 있습니다.
    hasher.update(data);

    // 최종 해시 계산 결과를 finalize() 메서드로 받아옵니다.
    // 결과는 GenericArray 타입으로 반환되므로, 이를 Rust의 표준 32바이트 고정 배열로 변환하기 위해
    // into() 메서드를 호출합니다.
    let result = hasher.finalize();
    let mut hash_bytes = [0u8; 32];
    hash_bytes.copy_from_slice(&result);

    hash_bytes
}

/// 32바이트 SHA-256 해시 배열을 사람이 읽기 편한 64글자의 16진수(hexadecimal) 문자열로 변환합니다.
///
/// # Rust 개념 설명:
/// - `String`: 힙(Heap) 메모리에 할당되는 동적 크기의 UTF-8 텍스트 문자열 타입입니다.
/// - `format!`: 문자열 포맷 출력을 돕는 편리한 매크로입니다. C언어의 `sprintf`와 유사합니다.
pub fn bytes_to_hex(bytes: &[u8; 32]) -> String {
    let mut hex_string = String::with_capacity(64);
    for byte in bytes {
        // {:02x}는 1바이트 값을 2자리 소문자 16진수로 포맷팅하라는 의미입니다. (예: 9 -> 09, 10 -> 0a)
        hex_string.push_str(&format!("{byte:02x}"));
    }
    hex_string
}
