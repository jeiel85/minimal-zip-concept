use crate::error::MzcError;

// [Rust 기초 설명]
// - const: 프로그램 실행 중 변하지 않는 '상수(Constant)'를 정의합니다.
// - usize: 컴퓨터 아키텍처(32비트 또는 64비트)에 맞는 크기를 가지는 부호 없는 정수 타입입니다. 주로 배열의 인덱스나 크기 표현에 사용됩니다.
// - 256KB 크기의 직접 매핑(Direct-Mapped) 해시 테이블 크기 정의 (2^18 = 262,144개 항목)
const C2_SIZE: usize = 262144;

/// **컨텍스트 믹싱(Context Mixing) 예측 모델 구조체**
///
/// [알고리즘 및 Rust 문법 설명]
/// - 컨텍스트 믹싱은 현재 압축 또는 해제 중인 비트가 '0'일지 '1'일지 예측하기 위해,
///   과거에 등장했던 데이터의 문맥(Context)을 3가지 레벨(0차, 1차, 2차)로 나누어 분석하고 이들의 예측 확률을 가중치 평균으로 섞는 기술입니다.
/// - struct: Rust에서 연관된 여러 데이터를 하나로 묶어 새로운 데이터 타입을 정의하는 '구조체' 선언 키워드입니다.
/// - Vec<(u8, u8)>: 
///   * Vec은 동적으로 크기가 변하는 가변 배열(Vector)입니다.
///   * (u8, u8)은 튜플(Tuple) 구조로, 2개의 8비트 부호 없는 정수(0~255 범위)를 한 쌍으로 묶은 것입니다.
///   * 각 튜플은 (현재 문맥에서 0이 나타난 횟수, 1이 나타난 횟수)를 누적 기록합니다.
pub struct CmModel {
    pub c0_table: Vec<(u8, u8)>,
    pub c1_table: Vec<(u8, u8)>,
    pub c2_table: Vec<(u8, u8)>,
    pub weights: [[i32; 3]; 8],
}

// [Rust 기초 설명]
// - impl (Implementation): 특정 구조체(struct)에 속하는 메서드나 생성자 함수를 작성하는 블록입니다.
impl CmModel {
    /// **CmModel 생성자 함수**
    ///
    /// [Rust 기초 설명]
    /// - Self: impl을 정의하고 있는 현재 구조체 타입(여기서는 CmModel)을 가리키는 지시어입니다.
    /// - vec![(0, 0); 크기]: 모든 요소를 (0, 0)으로 채운 지정된 크기의 벡터를 생성하는 매크로(식)입니다.
    pub fn new() -> Self {
        Self {
            c0_table: vec![(0, 0); 256],
            c1_table: vec![(0, 0); 65536],
            c2_table: vec![(0, 0); C2_SIZE],
            weights: [[1024, 2048, 5120]; 8],
        }
    }

    /// **현재 문맥 상태를 바탕으로 다음 비트가 '0'일 확률을 0 ~ 4096 범위의 정수로 예측합니다.**
    ///
    /// [알고리즘 및 Rust 문법 설명]
    /// - &self: 구조체 내부 데이터를 읽기만 하고 수정하지 않는 "읽기 전용 참조(Immutable Reference)"입니다.
    /// - u16, u8: 각각 16비트, 8비트 크기의 부호 없는 정수입니다.
    /// - u32: 32비트 크기의 부호 없는 정수입니다.
    /// - as usize / as u32: Rust의 엄격한 타입 체크 때문에, 서로 다른 크기의 정수나 인덱스를 연산하려면 명시적으로 형변환(Casting)을 해야 합니다.
    pub fn get_probability(&self, ctx_byte: u16, prev_byte_1: u8, prev_byte_2: u8, bit_idx: usize) -> u32 {
        // --- 1. Context 0 (0차 예측) ---
        // 현재 바이트 내에서 디코딩된 비트 경로(ctx_byte)를 인덱스로 삼아 0과 1의 빈도수를 가져옵니다.
        let idx0 = ctx_byte as usize;
        let (n0_0, n0_1) = self.c0_table[idx0];
        
        // [수학 - Laplace Smoothing (라플라스 스무딩)]
        // 빈도수가 0인 경우 확률이 완전히 0% 또는 100%가 되는 것을 방지하여 부호화 불가 상황을 방어합니다.
        // 분자에 +1, 분모에 +2를 더하여 기본 확률을 50% 부근으로 안전하게 유도한 후,
        // Range Coding을 위해 정수 범위(4096 곱하기)로 조율합니다.
        let p0 = ((n0_0 as u32 + 1) * 4096) / (n0_0 as u32 + n0_1 as u32 + 2);

        // --- 2. Context 1 (1차 문맥 예측) ---
        // 직전 1바이트 값을 상위 8비트로 밀고(<< 8), 현재 상태(ctx_byte)를 하위 8비트에 붙여 16비트 크기의 고유 인덱스를 만듭니다.
        // `|` 연산자는 비트 OR 연산자로 두 값을 합칩니다.
        let idx1 = ((prev_byte_1 as usize) << 8) | (ctx_byte as usize);
        let (n1_0, n1_1) = self.c1_table[idx1];
        let p1 = ((n1_0 as u32 + 1) * 4096) / (n1_0 as u32 + n1_1 as u32 + 2);

        // --- 3. Context 2 (2차 문맥 예측 - 해시 기반 직접 매핑) ---
        // 직전 2개 바이트와 현재 상태를 비트 연산으로 엮어 24비트 정수를 만들고,
        // 이를 테이블 크기(C2_SIZE)로 나눈 나머지(`%` 연산자)를 해시값으로 사용하여 한정된 크기의 테이블에 매핑합니다.
        let hash_val = (((prev_byte_2 as usize) << 16) | ((prev_byte_1 as usize) << 8) | (ctx_byte as usize)) % C2_SIZE;
        let (n2_0, n2_1) = self.c2_table[hash_val];
        let p2 = ((n2_0 as u32 + 1) * 4096) / (n2_0 as u32 + n2_1 as u32 + 2);

        // --- 4. 확률 혼합 (LMS Adaptive Mixing) ---
        // 고정 가중치 대신 현재 비트 위치(bit_idx)에 따른 적응형 가중치를 사용합니다.
        let w = self.weights[bit_idx];
        let sum_w = (w[0] + w[1] + w[2]) as u32;
        let mut p = (w[0] as u32 * p0 + w[1] as u32 * p1 + w[2] as u32 * p2) / sum_w;

        // 경계값 예외 처리: 확률이 완전히 0이거나 4096이 되면 레인지 분할 구간이 사라져 수학적 오류가 납니다.
        // 따라서 최솟값 1, 최댓값 4095로 제한(Clamping)합니다.
        if p == 0 {
            p = 1;
        } else if p >= 4096 {
            p = 4095;
        }
        p
    }

    /// **실제 인코딩/디코딩된 비트 결과를 예측 모델에 피드백하여 출현 빈도 테이블을 실시간 업데이트합니다.**
    ///
    /// [알고리즘 및 Rust 문법 설명]
    /// - &mut self: 구조체 내부 변수를 직접 수정할 수 있는 "가변 참조(Mutable Reference)"입니다.
    /// - 적응형 학습(Adaptive Learning): 데이터 스트림을 순차적으로 읽으면서 예측 가중치가 실시간으로 조율됩니다.
    pub fn update(&mut self, ctx_byte: u16, prev_byte_1: u8, prev_byte_2: u8, bit_idx: usize, bit: bool) {
        // --- 1. 가중치 학습을 위해 개별 확률을 우선 구합니다 ---
        let idx0 = ctx_byte as usize;
        let (n0_0, n0_1) = self.c0_table[idx0];
        let p0 = ((n0_0 as u32 + 1) * 4096) / (n0_0 as u32 + n0_1 as u32 + 2);

        let idx1 = ((prev_byte_1 as usize) << 8) | (ctx_byte as usize);
        let (n1_0, n1_1) = self.c1_table[idx1];
        let p1 = ((n1_0 as u32 + 1) * 4096) / (n1_0 as u32 + n1_1 as u32 + 2);

        let hash_val = (((prev_byte_2 as usize) << 16) | ((prev_byte_1 as usize) << 8) | (ctx_byte as usize)) % C2_SIZE;
        let (n2_0, n2_1) = self.c2_table[hash_val];
        let p2 = ((n2_0 as u32 + 1) * 4096) / (n2_0 as u32 + n2_1 as u32 + 2);

        let w = self.weights[bit_idx];
        let sum_w = (w[0] + w[1] + w[2]) as u32;
        let mut p = (w[0] as u32 * p0 + w[1] as u32 * p1 + w[2] as u32 * p2) / sum_w;
        if p == 0 {
            p = 1;
        } else if p >= 4096 {
            p = 4095;
        }

        // --- 2. 오차 계산 및 LMS 적응형 가중치 조정 ---
        // 비트가 0(false)이면 타겟 확률은 4096, 1(true)이면 타겟 확률은 0입니다.
        let target = if !bit { 4096i32 } else { 0i32 };
        let err = target - p as i32;
        
        let learning_shift = 15;
        for i in 0..3 {
            let pi_val = match i {
                0 => p0 as i32,
                1 => p1 as i32,
                2 => p2 as i32,
                _ => unreachable!(),
            };
            // 델타 업데이트 계산 및 가중치 업데이트
            let delta = (err * (pi_val - p as i32)) >> learning_shift;
            self.weights[bit_idx][i] = (self.weights[bit_idx][i] + delta).clamp(128, 16384);
        }

        // [Rust 문법 - 클로저(Closure, 익명 함수)]
        // - `|c: &mut (u8, u8), bit_val: bool| { ... }` 형태로 정의된 한 줄짜리 헬퍼 클로저입니다.
        // - 인자로 넘겨진 테이블 항목 `c`의 내부 값을 직접 갱신합니다.
        let update_entry = |c: &mut (u8, u8), bit_val: bool| {
            if !bit_val {
                // 들어온 비트가 false(즉, 0)이면 0 카운터 증가 (최대값 255 한계 설정)
                if c.0 < 255 {
                    c.0 += 1;
                }
            } else {
                // 들어온 비트가 true(즉, 1)이면 1 카운터 증가
                if c.1 < 255 {
                    c.1 += 1;
                }
            }
            
            // [슬라이딩 윈도우 스케일링]
            // 만약 0과 1의 빈도수의 합이 120을 초과하면 두 빈도수를 모두 절반으로 줄입니다(우측 쉬프트 >> 1).
            // 오래된 과거 데이터의 가중치를 점진적으로 희석하고 최신 경향성을 더 잘 따르도록 만들기 위한 감쇠 필터입니다.
            // .max(1)은 0이 되어 정보가 완전히 소실되는 것을 예방하기 위해 최솟값을 1로 강제합니다.
            if c.0 as u16 + c.1 as u16 > 120 {
                c.0 = (c.0 >> 1).max(1);
                c.1 = (c.1 >> 1).max(1);
            }
        };

        // 0차 문맥 업데이트
        update_entry(&mut self.c0_table[idx0], bit);

        // 1차 문맥 업데이트
        update_entry(&mut self.c1_table[idx1], bit);

        // 2차 문맥 업데이트
        update_entry(&mut self.c2_table[hash_val], bit);
    }
}

/// **레인지 인코더(Range Encoder) 구조체**
///
/// [알고리즘 및 Rust 문법 설명]
/// - 실수 수직선 영역 [0.0, 1.0)을 데이터 모델의 비트 확률 비율에 맞춰 하위 구간으로 쪼개나가며,
///   구간이 좁혀질 때마다 확정된 상위 자릿수 바이트를 출력 스트림으로 내보내 압축하는 정밀 산술 코더(Arithmetic Coder)입니다.
struct RangeEncoder {
    // 현재 인코딩 구간의 하한선 (오버플로우 및 캐리 계산 처리를 위해 64비트 정수로 크게 잡습니다)
    low: u64,
    // 현재 인코딩 영역의 폭(길이) (초기값은 최댓값인 0xFFFF_FFFF)
    range: u32,
    // 정밀도 복원을 위해 방출을 일시 보류하고 대기 중인 바이트들의 개수
    cache_size: u64,
    // 대기 중인 바로 이전의 방출 후보 바이트
    cache: u8,
    // 압축된 최종 바이트 스트림이 순차적으로 저장될 벡터
    out: Vec<u8>,
}

impl RangeEncoder {
    // 새로운 레인지 인코더 인스턴스를 기본 상태로 초기 설정하여 반환합니다.
    fn new() -> Self {
        Self {
            low: 0,
            range: 0xFFFF_FFFF,
            cache_size: 1,
            cache: 0,
            out: Vec::new(),
        }
    }

    /// **예측 확률 `p`를 바탕으로 비트 1개를 구간 분할하여 인코딩합니다.**
    ///
    /// # 작동 원리:
    /// - `p`: 다음 비트가 0일 확률을 나타내며, 0~4096 사이의 정수입니다.
    /// - 현재의 `range`(구간 폭)를 `p / 4096` 비율로 쪼개어 경계선 `boundary`를 결정합니다.
    /// - 비트가 0(false)이면 하위 영역을 타겟팅하고, 1(true)이면 상위 영역을 타겟팅하여 새로운 구간을 좁힙니다.
    fn encode_bit(&mut self, bit: bool, p: u32) {
        // 구간을 p / 4096 비율로 정밀 분할한 경계값 계산 (64비트 정수 연산 후 32비트로 타입 전환)
        let boundary = ((self.range as u64 * p as u64) >> 12) as u32;
        if !bit {
            // 비트가 0인 경우: 하한선(low)은 그대로 유지하고, 구간 크기(range)를 boundary 크기로 좁힙니다.
            self.range = boundary;
        } else {
            // 비트가 1인 경우: 하한선(low)을 boundary만큼 올리고, 구간 크기(range)는 남은 영역만큼 좁힙니다.
            self.low += boundary as u64;
            self.range -= boundary;
        }
        
        // [리노멀라이즈 (Renormalize)]
        // 구간 크기(range)가 24비트 크기(0x0100_0000)보다 작아지면, 부동소수점 수준의 정밀도 한계를 넘어가므로
        // 확정된 상위 자릿수 바이트들을 외부 스트림으로 출력(shift_low)하고 구간 범위를 256배 좌측 쉬프트하여 정밀도를 재확보합니다.
        while self.range < 0x0100_0000 {
            self.shift_low();
        }
    }

    /// **리노멀라이즈 도중 캐리(반올림) 전파를 안전하게 고려하여 최상위 바이트들을 스트림에 씁니다.**
    ///
    /// # Rust 개념 및 동작 설명:
    /// - `wrapping_add`: Rust는 기본 덧셈 시 정수 오버플로우가 나면 패닉(강제종료)을 내므로,
    ///   255를 넘어설 때 0으로 자연스럽게 순환하도록 방지 코드를 사용합니다. (255 + 1 = 0)
    /// - 캐리 전파 방어 기법:
    ///   * 출력할 다음 바이트가 `0xFF`인 경우, 이후 단계에서 캐리(올림)가 넘어오면 이 `0xFF`가 `0x00`으로 바뀔 수 있습니다.
    ///   * 따라서, 다음 바이트가 `0xFF`일 때는 출력을 즉시 내보내지 않고 `cache_size` 변수만 1 증가시켜 보류합니다.
    ///   * 만약 다음 바이트가 `0xFF`보다 작거나, 또는 이미 자리올림(low >= 0x01_0000_0000)이 일어나 캐리 전파 여부가 확정되면
    ///     보류해 둔 바이트들을 상황에 따라 `0x00` 혹은 `0xFF`로 결정하여 한꺼번에 밀어내어 안전하게 방출합니다.
    fn shift_low(&mut self) {
        // 출력될 예정인 다음 바이트(24~31번째 비트)를 미리 발라내어 확인합니다.
        let next_byte = (self.low >> 24) as u8;
        
        // 캐리가 이미 확실하게 발생했거나, 다음 바이트가 0xFF가 아니어서 나중에 캐리가 전입되어도 더는 위로 전파되지 않는 경우
        if next_byte < 0xFF || self.low >= 0x01_0000_0000 {
            let mut c = self.cache;
            // low의 최상위 오버플로우 캐리 비트(32번째 비트 이상)를 cache에 더해 반올림(캐리)을 마저 처리합니다.
            c = c.wrapping_add((self.low >> 32) as u8);
            self.out.push(c);
            
            // 그동안 0xFF가 나와서 방출을 일시 보류하고 카운트만 해 두었던 바이트들을 몽땅 외부 버퍼로 밀어냅니다.
            // 실제로 캐리가 최종 발생했다면 이 보류 바이트들은 0이 되고, 아니라면 원래대로 0xFF가 되어 기록됩니다.
            for _ in 0..self.cache_size - 1 {
                self.out.push(if self.low >= 0x01_0000_0000 { 0 } else { 0xFF });
            }
            
            // 다음 캐리 판단을 위해 cache 위치를 현재 다음 바이트 값으로 세팅하고 대기 카운터를 1로 돌려놓습니다.
            self.cache = next_byte;
            self.cache_size = 1;
        } else {
            // 다음 방출 예정 바이트가 0xFF이면, 캐리가 나중에 확정될 때까지 임시 대기시킵니다.
            self.cache_size += 1;
        }
        
        // low의 상위 8비트를 처리 완료했으므로 비워내고(& 0x00FF_FFFF)
        // 왼쪽으로 8비트 쉬프트(<< 8)하여 다음 바이트 공간을 만듭니다. range 또한 8비트 곱합니다.
        self.low = (self.low & 0x00FF_FFFF) << 8;
        self.range <<= 8;
    }

    /// **인코딩이 모두 끝난 후, 구간 내에 잔존해 있는 바이트 잔량을 강제로 내보내 압축 출력을 마무리합니다.**
    fn finish(&mut self) {
        // 충분한 정밀도 바이트들이 모두 밀려 나가도록 5회 호출하여 flush를 완수합니다.
        for _ in 0..5 {
            self.shift_low();
        }
    }
}

/// **레인지 디코더(Range Decoder) 구조체**
///
/// [알고리즘 및 Rust 문법 설명]
/// - &'a [u8]: 라이프타임 기호 `'a`가 선언되어 있습니다. 
///   이는 참조 대상인 바이트 슬라이스(`bytes`)가 디코더 인스턴스보다 메모리 상에 길게 잔존함을 나타내는 안정성 증명 표식입니다.
struct RangeDecoder<'a> {
    // 디코더 측의 구간 크기 (초기값은 최대폭 0xFFFF_FFFF)
    range: u32,
    // 현재 읽은 비트 스트림의 실수 표현 코딩 포인트 버퍼 값
    code: u32,
    // 압축된 이진 데이터를 가리키는 슬라이스
    bytes: &'a [u8],
    // 현재 bytes 배열의 몇 번째 바이트를 읽고 있는지 나타내는 포인터 인덱스
    pos: usize,
}

impl<'a> RangeDecoder<'a> {
    /// **RangeDecoder 생성 및 초기 32비트 코드 버퍼 채우기**
    ///
    /// [Rust 기초 설명]
    /// - Result<Self, MzcError>: 
    ///   성공 시 자기 자신(Self, 즉 RangeDecoder)을 반환하고,
    ///   실패 시 정의된 사용자 정의 에러 타입 `MzcError`를 감싸 반환하는 Rust의 표준 반환 열거형입니다.
    fn new(bytes: &'a [u8]) -> Result<Self, MzcError> {
        // 압축 코드 데이터가 너무 짧으면 32비트 버퍼 조립조차 불가능하므로 에러를 리턴합니다.
        if bytes.len() < 5 {
            return Err(MzcError::TruncatedBlock { expected: 5, found: bytes.len() });
        }
        let mut dec = Self {
            range: 0xFFFF_FFFF,
            code: 0,
            bytes,
            pos: 0,
        };
        // 스트림의 최초 5바이트를 순차적으로 읽어와 32비트 코드 포인트(code)를 초기 조립합니다.
        for _ in 0..5 {
            let b = if dec.pos < bytes.len() { bytes[dec.pos] } else { 0 };
            dec.code = (dec.code << 8) | b as u32;
            dec.pos += 1;
        }
        Ok(dec)
    }

    /// **예측 확률 `p`를 사용하여 다음 1비트를 참(1) 또는 거짓(0)으로 역분할 복원합니다.**
    fn decode_bit(&mut self, p: u32) -> bool {
        // 인코더와 완벽히 수학적으로 동기화된 방식으로 경계 분할 비율 boundary를 계산합니다.
        let boundary = ((self.range as u64 * p as u64) >> 12) as u32;
        
        if self.code < boundary {
            // 현재 수신된 코드 포인트가 분할 경계 미만이면, 원래 비트가 0(false)이었음을 역추론해 낼 수 있습니다.
            self.range = boundary;
            // 구간 크기가 낮아지면 리노멀라이즈하여 스트림으로부터 8비트 단위 바이트들을 끌어와 복원 정밀도를 충전합니다.
            while self.range < 0x0100_0000 {
                let b = if self.pos < self.bytes.len() { self.bytes[self.pos] } else { 0 };
                self.code = (self.code << 8) | b as u32;
                self.range <<= 8;
                self.pos += 1;
            }
            false
        } else {
            // 수신된 코드 포인트가 분할 경계 이상이면, 원래 비트가 1(true)이었음을 역추론해 냅니다.
            // 구간 폭과 코드 포인트를 boundary만큼 차감하고 동일하게 리노멀라이즈를 수행합니다.
            self.range -= boundary;
            self.code -= boundary;
            while self.range < 0x0100_0000 {
                let b = if self.pos < self.bytes.len() { self.bytes[self.pos] } else { 0 };
                self.code = (self.code << 8) | b as u32;
                self.range <<= 8;
                self.pos += 1;
            }
            true
        }
    }
}

/// **외부에서 호출하는 고성능 Context Mixing 압축의 핵심 진입점(Entry Point)입니다.**
///
/// # Rust 기초 설명:
/// - pub fn (Public Function): 이 함수가 외부 크레이트나 다른 소스코드 파일에서도 자유롭게 호출될 수 있음을 선언합니다.
/// - Result<Vec<u8>, MzcError>: 계산 완료 시 압축된 바이트 데이터 벡터(`Vec<u8>`)를 반환하고 실패 시 에러를 반환합니다.
pub fn cm_compress(data: &[u8]) -> Result<Vec<u8>, MzcError> {
    let mut encoder = RangeEncoder::new();
    let mut model = CmModel::new();

    // 문맥 상태 추적용 직전 1번째, 2번째 바이트 값 저장 변수 (초기값은 0)
    let mut prev_byte_1 = 0u8;
    let mut prev_byte_2 = 0u8;

    for &byte in data {
        // `ctx_byte`: 이진 트리 탐색 경로처럼 현재 바이트 내의 비트 누적 상태를 노드 주소(1~255)형태로 기억하는 포인터입니다.
        // 매 바이트 인코딩 시작 시 트리 루트인 1로 초기 세팅합니다.
        let mut ctx_byte = 1u16;
        for i in (0..8).rev() {
            // 해당 바이트의 최상위 비트(7번)부터 최하위 비트(0번)까지 순차적으로 끄집어냅니다.
            let bit = ((byte >> i) & 1) != 0;
            let bit_idx = (7 - i) as usize;
            
            // 1. 모델로부터 현재 문맥 상태에 기초한 다음 비트의 0일 예측 확률(p)을 쿼리합니다.
            let p = model.get_probability(ctx_byte, prev_byte_1, prev_byte_2, bit_idx);
            
            // 2. 알아낸 비트값과 예측된 확률 분포를 산술 레인지 인코더에 공급하여 공간 압축을 진행합니다.
            encoder.encode_bit(bit, p);
            
            // 3. 방금 인코딩된 실제 비트값을 사용하여 예측 통계값 모델을 실시간 학습 갱신(update)시킵니다.
            model.update(ctx_byte, prev_byte_1, prev_byte_2, bit_idx, bit);
            
            // 4. `ctx_byte`에 현재 비트를 비트 쉬프트로 밀어넣어 다음 비트 예측을 위한 트리 상태를 완성합니다.
            ctx_byte = (ctx_byte << 1) | (bit as u16);
        }
        // 한 바이트 인코딩이 완전히 끝났으므로 직전 바이트 레코드 정보를 한 단계 시프트 갱신합니다.
        prev_byte_2 = prev_byte_1;
        prev_byte_1 = byte;
    }

    // 인코더 버퍼에 남아있는 잔여 비압축 실수 값들을 모두 밀어내어 패킹을 끝냅니다.
    encoder.finish();
    Ok(encoder.out)
}

/// **외부에서 호출하는 Context Mixing 압축 바이트 해제(Decompress) 복원 진입점입니다.**
///
/// 압축 완료된 바이트 배열(`cm_bytes`)과 원래 원본 파일 크기(`original_size`)를 받아
/// 역으로 비트를 복조하고 온전한 원본 바이트 배열로 다시 엮어냅니다.
pub fn cm_decompress(cm_bytes: &[u8], original_size: usize) -> Result<Vec<u8>, MzcError> {
    if original_size == 0 {
        return Ok(Vec::new());
    }

    // 디코더 및 문맥 통계 모델을 초기 배정합니다.
    let mut decoder = RangeDecoder::new(cm_bytes)?;
    let mut model = CmModel::new();

    let mut prev_byte_1 = 0u8;
    let mut prev_byte_2 = 0u8;
    
    // 복원할 크기만큼 미리 벡터 용량 메모리를 사전 예약(Vec::with_capacity)하여 메모리 재할당 오버헤드를 막습니다.
    let mut out = Vec::with_capacity(original_size);

    for _ in 0..original_size {
        let mut byte = 0u8;
        let mut ctx_byte = 1u16;
        for i in 0..8 {
            let bit_idx = i;
            
            // 1. 압축 시와 동일하게 0~2차 정보 문맥을 조회하여 비트 확률(p)을 산출합니다.
            let p = model.get_probability(ctx_byte, prev_byte_1, prev_byte_2, bit_idx);
            
            // 2. 레인지 디코더의 실수 분할 지점을 대조하여 실제 부호화되었던 원본 비트(bool)를 판정 복원합니다.
            let bit = decoder.decode_bit(p);
            
            // 3. 복원된 비트를 왼쪽으로 계속 밀어서 한 바이트(8비트) 형태로 조각들을 퍼즐처럼 완성합니다.
            byte = (byte << 1) | (bit as u8);
            
            // 4. 복조 완료된 비트를 활용하여 인코더측 통계 모델과 어긋남이 없도록 학습 통계 테이블을 동일하게 동기식 업데이트합니다.
            model.update(ctx_byte, prev_byte_1, prev_byte_2, bit_idx, bit);
            ctx_byte = (ctx_byte << 1) | (bit as u16);
        }
        // 완성된 1바이트를 복원 스트림 버퍼에 추가합니다.
        out.push(byte);
        
        // 직전 문자 기록 장치를 갱신합니다.
        prev_byte_2 = prev_byte_1;
        prev_byte_1 = byte;
    }

    Ok(out)
}

// [Rust 기초 설명]
// - #[cfg(test)]: cargo test 명령어를 실행할 때만 컴파일되어 작동하는 독립 테스트 모듈 빌드 설정입니다.
#[cfg(test)]
mod tests {
    use super::*;

    // - #[test]: 이 함수가 단위 테스트 케이스(Unit Test Case) 중 하나임을 알려주는 어노테이션입니다.
    #[test]
    fn test_cm_direct() {
        let inputs: &[&[u8]] = &[
            b"Hello, Context Mixing!",
            b"AAAAHello! This is a repeated text BBBB test. ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ!",
            &[0u8; 1000],
            &[b'A'; 70000],
        ];
        for (i, input) in inputs.iter().enumerate() {
            let compressed = cm_compress(input).unwrap();
            println!("Input {}, Compressed length: {}", i, compressed.len());
            let decompressed = cm_decompress(&compressed, input.len()).unwrap();
            // assert_eq!는 양쪽 매개변수 값이 일치하지 않는 경우 오류 에러를 보고하여 테스트를 실패시킵니다.
            assert_eq!(*input, decompressed.as_slice(), "Failed on input {}", i);
        }
    }
}
