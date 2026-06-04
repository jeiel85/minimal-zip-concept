// 이 파일은 오디오 및 이미지와 같은 미디어 파일들의 패턴화된 중복성을 사전 필터링하여 
// 압축 효율을 극대화시키는 전처리 예측 필터(Preprocessor Filter)를 정의한 모듈입니다.
// Rust를 한 번도 공부해보지 않은 입문자분들도 원리와 코드를 직관적으로 읽을 수 있도록 세밀하게 주석을 달아 놓았습니다.

/// **PNG 이미지 규격에서 활용되는 Paeth(페스) 예측기 알고리즘**
///
/// [알고리즘 설명]
/// - 이미지 픽셀은 가로, 세로 방향으로 유사한 색상이 부드럽게 흐르는 경향이 많습니다.
/// - 따라서 현재 픽셀(x)의 색상을 예측하기 위해 왼쪽(a), 위쪽(b), 대각선 왼쪽 위(c) 픽셀 값을 관찰합니다.
/// - 수학적 관계식 `p = a + b - c`을 계산하고, 이 `p` 값에 가장 가깝고 거리가 짧은 픽셀의 원본 색상(a, b, c 중 하나)을 
///   현재 예측 색상으로 간주해 그 차이값(잔차, Residual)만 압축 스트림으로 부호화합니다.
///
/// [Rust 기초 설명]
/// - u8: 1바이트 크기(8비트)의 부호 없는 정수형 타입으로 0~255 범위의 값을 가집니다. (주로 원시 이미지 바이트 표기)
/// - i32: 4바이트 크기(32비트)의 부호 있는 정수형 타입입니다.
/// - as i32: Rust는 정적 타입 강제 검사를 수행하므로, 8비트 정수끼리 더하다가 255를 넘겨 에러가 나거나
///   뺄셈 중 음수가 되어 폭주하는 것을 미리 예방하기 위해, 안전하게 32비트 크기인 `i32` 타입으로 크기를 부풀려 연산합니다.
fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    // 픽셀 간의 변화량을 반영하는 기본 예측 포인트 `p`를 정수형으로 구합니다.
    let p = a as i32 + b as i32 - c as i32;
    
    // `.abs()`: 절대값(Absolute Value)을 반환하는 정수 메서드입니다. 
    // 예측 포인트 `p`로부터 각 주변 픽셀들(a, b, c)까지의 직선적 거리를 구합니다.
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    
    // [Rust 문법 특징 - if-else의 식(Expression) 동작 방식]
    // Rust에서 `if-else`문은 단순히 코드 제어 흐름만 나누는 것이 아니라, 
    // 각 블록의 마지막 줄에 세미콜론 없이 변수나 값을 두면 그 자체를 결과값으로 통째로 리턴합니다.
    if pa <= pb && pa <= pc {
        a // a가 p에 가장 가까우므로 a 값을 반환
    } else if pb <= pc {
        b // b가 p에 가장 가까우므로 b 값을 반환
    } else {
        c // c가 p에 가장 가까우므로 c 값을 반환
    }
}

/// **PNG 스타일의 Paeth 필터를 이미지 바이트 배열 전체에 적용(인코딩)합니다.**
///
/// [알고리즘 및 Rust 문법 설명]
/// - &mut [u8]: 
///   * `&`는 주소값을 넘겨주는 참조자(Reference)입니다.
///   * `mut`는 이 함수 내부에서 해당 변수 안의 데이터 값을 수정(Mutate)할 수 있도록 허락해 주는 키워드입니다.
///   * `[u8]`은 바이트의 크기가 정해지지 않은 메모리 연속 영역(슬라이스)을 나타냅니다.
pub fn apply_png_filter(data: &mut [u8]) {
    // 이미지의 한 행(가로 폭)의 크기를 2048바이트로 상정합니다. (표준 폭 규격)
    let width = 2048;
    
    // [Rust 기초 설명 - 메모리 소유권과 복사]
    // - `data.to_vec()`: 원본 data 슬라이스가 가리키는 실제 데이터를 힙(Heap) 공간에 완벽히 복사하여 
    //   새로운 동적 배열 벡터 `Vec<u8>`를 만듭니다.
    // - 앞부분 픽셀 필터링 결과가 뒷부분 연산에 혼선을 주는 것을 막기 위해 원본 상태의 복제본 `orig`이 필요합니다.
    let orig = data.to_vec();
    
    // `0..data.len()`: 0부터 data 배열의 전체 크기 직전까지 1씩 순차적으로 전진하는 반복 범위 루프입니다.
    for i in 0..data.len() {
        // [경계 조건 체크와 주변 픽셀 조회]
        // - `i % width`: 현재 바이트 인덱스가 줄(Row)의 몇 번째 열인지 체크합니다.
        // - RGBA 4채널 픽셀 색상 구성을 가용하므로 왼쪽 픽셀은 `i - 4`에 위치합니다.
        // - 경계를 넘어가거나 첫 줄인 경우 데이터가 존재하지 않으므로 디폴트값인 0으로 대체합니다.
        let left = if i >= 4 && (i % width) >= 4 { orig[i - 4] } else { 0 };
        let up = if i >= width { orig[i - width] } else { 0 };
        let upleft = if i >= width + 4 && (i % width) >= 4 { orig[i - width - 4] } else { 0 };
        
        // [수학 - wrapping_sub]
        // - Rust는 뺄셈 결과가 0 아래(음수)로 내려갈 때 코드가 폭주하여 죽는(Panic) 것을 기본 방어합니다.
        // - wrapping_sub는 0 이하로 내려갈 때 255 방향으로 감돌아 정상 뺄셈이 유지되도록 순환 처리를 합니다. (예: 5 - 10 = 251)
        data[i] = orig[i].wrapping_sub(paeth_predictor(left, up, upleft));
    }
}

/// **apply_png_filter에 의해 픽셀 예측값과의 잔차로 변경된 데이터를 원본 색상 데이터로 복원(디코딩)합니다.**
///
/// [알고리즘 설명]
/// - 압축 해제 시에는 왼쪽과 위쪽 픽셀들이 앞에서부터 이미 온전히 제자리 색상으로 다 복원되어 있습니다.
/// - 따라서 별도의 복제 가변 벡터를 추가 할당할 필요 없이, 앞에서부터 순서대로 원본을 자기 참조 복원해 갈 수 있습니다.
pub fn inverse_png_filter(data: &mut [u8]) {
    let width = 2048;
    
    for i in 0..data.len() {
        let left = if i >= 4 && (i % width) >= 4 { data[i - 4] } else { 0 };
        let up = if i >= width { data[i - width] } else { 0 };
        let upleft = if i >= width + 4 && (i % width) >= 4 { data[i - width - 4] } else { 0 };
        
        // [수학 - wrapping_add]
        // - 덧셈 결과가 255를 초과해 자리넘침이 발생하면 다시 0부터 회전하여 덧셈을 완수합니다. (wrapping_sub와 정확한 대칭 관계)
        data[i] = data[i].wrapping_add(paeth_predictor(left, up, upleft));
    }
}

/// **16비트 오디오 PCM(WAV) 데이터를 위한 LPC(선형 예측 부호화) 전처리 필터 (인코딩)**
///
/// [알고리즘 설명]
/// - 소리 파형 데이터(Audio Signal)는 급격히 튀지 않고 인접 샘플들이 사인파 곡선 흐름처럼 연결되는 성질이 강합니다.
/// - 이를 모델링하여 2차 예측(Order-2 Linear Prediction) 모델을 설계합니다: `예측값 = 2 * 직전샘플 - 2단계직전샘플`
/// - 즉, 직전 두 개 샘플의 변화 흐름과 가속도를 그대로 반영해 다음 오디오 샘플 값을 예측하고 그 예측 편차(잔차)만 모아 압축기로 흘립니다.
pub fn apply_lpc_filter(data: &mut [u8]) {
    // 16비트 오디오 샘플은 개당 2바이트를 소모하므로 전체 바이트 크기를 2로 나누어 총 샘플 수 `n`을 산출합니다.
    let n = data.len() / 2;
    if n < 2 {
        return; // 오디오 데이터가 너무 짧아 샘플이 2개 미만인 경우 예측 필터 적용이 생략됩니다.
    }
    
    // [Rust 기초 설명]
    // - vec![0i16; n]: 16비트 부호 있는 정수(`i16`) 타입의 메모리 소형 벡터를 0으로 꽉 채워 n의 크기로 생성합니다.
    let mut samples = vec![0i16; n];
    for i in 0..n {
        // WAV 포맷의 리틀 엔디안(Little-Endian, 하위 바이트 우선 배치) 바이트 스트림을 파싱합니다.
        let b0 = data[2 * i];       // 하위 8비트
        let b1 = data[2 * i + 1];   // 상위 8비트
        
        // `i16::from_le_bytes`: 2바이트 배열 `[u8; 2]`을 입력받아 정밀 16비트 부호 있는 음성 정수로 재합성합니다.
        samples[i] = i16::from_le_bytes([b0, b1]);
    }
    
    // 필터링 도중 앞선 잔차 결과가 연쇄 왜곡을 주지 않도록 오디오 데이터 사본을 확보합니다.
    // `.clone()`: 깊은 복사(Deep Copy)를 통해 완전히 독립된 별개의 벡터 인스턴스를 하나 복제합니다.
    let orig = samples.clone();
    
    let mut i = 2;

    // x86_64 SSE2 하드웨어 가속
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sse2") {
            while i + 7 < n {
                unsafe {
                    use std::arch::x86_64::*;
                    let val_i = _mm_loadu_si128(orig[i..].as_ptr() as *const __m128i);
                    let val_prev1 = _mm_loadu_si128(orig[i-1..].as_ptr() as *const __m128i);
                    let val_prev2 = _mm_loadu_si128(orig[i-2..].as_ptr() as *const __m128i);

                    let two_prev1 = _mm_add_epi16(val_prev1, val_prev1);
                    let pred = _mm_sub_epi16(two_prev1, val_prev2);
                    let res = _mm_sub_epi16(val_i, pred);

                    _mm_storeu_si128(samples[i..].as_mut_ptr() as *mut __m128i, res);
                }
                i += 8;
            }
        }
    }
    
    // 2번 샘플 위치부터 시작하여 끝까지 2차 선형 예측 예측값을 빼나갑니다. (SIMD 루프 이후 잔여물 처리)
    for j in i..n {
        // 오디오 샘플 연산 중 자리넘침을 예방하기 위해 i32 크기로 승격시킵니다.
        let pred = 2 * orig[j - 1] as i32 - orig[j - 2] as i32;
        
        // 실제 오디오 파형 값에서 수학적으로 예측된 파형 값을 빼서 잔차를 계산합니다.
        // wrapping_sub을 수행하여 안전한 범위 정수 회전 뺄셈을 관철합니다.
        samples[j] = (orig[j] as i32).wrapping_sub(pred) as i16;
    }
    
    // 도출된 예측 편차(잔차) 오디오 값들을 다시 리틀 엔디안 바이트 형태로 변환하여 최종 압축 데이터 버퍼에 써 넣습니다.
    for i in 0..n {
        // `.to_le_bytes()`: 16비트 소리 정수 값을 다시 [u8; 2] 규격의 2바이트 배열로 갈라 줍니다.
        let bytes = samples[i].to_le_bytes();
        data[2 * i] = bytes[0];
        data[2 * i + 1] = bytes[1];
    }
}

/// **apply_lpc_filter에 의해 잔차 신호로 분해된 오디오 스트림을 원래의 16비트 PCM 음향 파형 데이터로 복원합니다.**
///
/// [알고리즘 설명]
/// - 압축 해제 시에는 이전 오디오 파형 샘플들(i-1, i-2)이 시간적 순서에 맞춰 이미 앞단에서 정상 파형으로 재구축되어 있습니다.
/// - 따라서 마찬가지로 별도 메모리 복제 없이 2번 인덱스부터 순방향으로 역예측 연산을 거쳐 복구합니다.
pub fn inverse_lpc_filter(data: &mut [u8]) {
    let n = data.len() / 2;
    if n < 2 {
        return;
    }
    
    let mut samples = vec![0i16; n];
    for i in 0..n {
        let b0 = data[2 * i];
        let b1 = data[2 * i + 1];
        samples[i] = i16::from_le_bytes([b0, b1]);
    }
    
    // 역연산: 뺄셈했던 예측값을 다시 덧셈(wrapping_add)하여 원래의 파형을 완벽히 도출해 냅니다.
    for i in 2..n {
        let pred = 2 * samples[i - 1] as i32 - samples[i - 2] as i32;
        samples[i] = samples[i].wrapping_add(pred as i16);
    }
    
    // 복원된 16비트 오디오 샘플들을 바이트 데이터로 분쇄 복원합니다.
    for i in 0..n {
        let bytes = samples[i].to_le_bytes();
        data[2 * i] = bytes[0];
        data[2 * i + 1] = bytes[1];
    }
}
