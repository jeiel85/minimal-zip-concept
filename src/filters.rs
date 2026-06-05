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

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn paeth_predictor_avx2(
    va: std::arch::x86_64::__m256i,
    vb: std::arch::x86_64::__m256i,
    vc: std::arch::x86_64::__m256i,
) -> std::arch::x86_64::__m256i {
    use std::arch::x86_64::*;
    let zero = _mm256_setzero_si256();

    let va_lo = _mm256_unpacklo_epi8(va, zero);
    let va_hi = _mm256_unpackhi_epi8(va, zero);

    let vb_lo = _mm256_unpacklo_epi8(vb, zero);
    let vb_hi = _mm256_unpackhi_epi8(vb, zero);

    let vc_lo = _mm256_unpacklo_epi8(vc, zero);
    let vc_hi = _mm256_unpackhi_epi8(vc, zero);

    // Process low 16 elements
    let p_lo = _mm256_sub_epi16(_mm256_add_epi16(va_lo, vb_lo), vc_lo);
    let pa_lo = _mm256_abs_epi16(_mm256_sub_epi16(p_lo, va_lo));
    let pb_lo = _mm256_abs_epi16(_mm256_sub_epi16(p_lo, vb_lo));
    let pc_lo = _mm256_abs_epi16(_mm256_sub_epi16(p_lo, vc_lo));

    let cond_pa_le_pb_lo = _mm256_cmpeq_epi16(_mm256_min_epi16(pa_lo, pb_lo), pa_lo);
    let cond_pa_le_pc_lo = _mm256_cmpeq_epi16(_mm256_min_epi16(pa_lo, pc_lo), pa_lo);
    let mask_a_lo = _mm256_and_si256(cond_pa_le_pb_lo, cond_pa_le_pc_lo);
    let cond_pb_le_pc_lo = _mm256_cmpeq_epi16(_mm256_min_epi16(pb_lo, pc_lo), pb_lo);

    let res_bc_lo = _mm256_blendv_epi8(vc_lo, vb_lo, cond_pb_le_pc_lo);
    let res_lo = _mm256_blendv_epi8(res_bc_lo, va_lo, mask_a_lo);

    // Process high 16 elements
    let p_hi = _mm256_sub_epi16(_mm256_add_epi16(va_hi, vb_hi), vc_hi);
    let pa_hi = _mm256_abs_epi16(_mm256_sub_epi16(p_hi, va_hi));
    let pb_hi = _mm256_abs_epi16(_mm256_sub_epi16(p_hi, vb_hi));
    let pc_hi = _mm256_abs_epi16(_mm256_sub_epi16(p_hi, vc_hi));

    let cond_pa_le_pb_hi = _mm256_cmpeq_epi16(_mm256_min_epi16(pa_hi, pb_hi), pa_hi);
    let cond_pa_le_pc_hi = _mm256_cmpeq_epi16(_mm256_min_epi16(pa_hi, pc_hi), pa_hi);
    let mask_a_hi = _mm256_and_si256(cond_pa_le_pb_hi, cond_pa_le_pc_hi);
    let cond_pb_le_pc_hi = _mm256_cmpeq_epi16(_mm256_min_epi16(pb_hi, pc_hi), pb_hi);

    let res_bc_hi = _mm256_blendv_epi8(vc_hi, vb_hi, cond_pb_le_pc_hi);
    let res_hi = _mm256_blendv_epi8(res_bc_hi, va_hi, mask_a_hi);
    _mm256_packus_epi16(res_lo, res_hi)
}

#[cfg(target_arch = "aarch64")]
unsafe fn paeth_predictor_neon(
    va: std::arch::aarch64::uint8x16_t,
    vb: std::arch::aarch64::uint8x16_t,
    vc: std::arch::aarch64::uint8x16_t,
) -> std::arch::aarch64::uint8x16_t {
    use std::arch::aarch64::*;

    let va_lo = vreinterpretq_s16_u16(vmovl_u8(vget_low_u8(va)));
    let va_hi = vreinterpretq_s16_u16(vmovl_high_u8(va));

    let vb_lo = vreinterpretq_s16_u16(vmovl_u8(vget_low_u8(vb)));
    let vb_hi = vreinterpretq_s16_u16(vmovl_high_u8(vb));

    let vc_lo = vreinterpretq_s16_u16(vmovl_u8(vget_low_u8(vc)));
    let vc_hi = vreinterpretq_s16_u16(vmovl_high_u8(vc));

    let p_lo = vsubq_s16(vaddq_s16(va_lo, vb_lo), vc_lo);
    let pa_lo = vabsq_s16(vsubq_s16(p_lo, va_lo));
    let pb_lo = vabsq_s16(vsubq_s16(p_lo, vb_lo));
    let pc_lo = vabsq_s16(vsubq_s16(p_lo, vc_lo));

    let cond_pa_le_pb_lo = vcleq_s16(pa_lo, pb_lo);
    let cond_pa_le_pc_lo = vcleq_s16(pa_lo, pc_lo);
    let mask_a_lo = vandq_u16(cond_pa_le_pb_lo, cond_pa_le_pc_lo);
    let cond_pb_le_pc_lo = vcleq_s16(pb_lo, pc_lo);

    let res_bc_lo = vreinterpretq_s16_u16(vbslq_u16(
        cond_pb_le_pc_lo,
        vreinterpretq_u16_s16(vb_lo),
        vreinterpretq_u16_s16(vc_lo),
    ));
    let res_lo = vreinterpretq_s16_u16(vbslq_u16(
        mask_a_lo,
        vreinterpretq_u16_s16(va_lo),
        vreinterpretq_u16_s16(res_bc_lo),
    ));

    let p_hi = vsubq_s16(vaddq_s16(va_hi, vb_hi), vc_hi);
    let pa_hi = vabsq_s16(vsubq_s16(p_hi, va_hi));
    let pb_hi = vabsq_s16(vsubq_s16(p_hi, vb_hi));
    let pc_hi = vabsq_s16(vsubq_s16(p_hi, vc_hi));

    let cond_pa_le_pb_hi = vcleq_s16(pa_hi, pb_hi);
    let cond_pa_le_pc_hi = vcleq_s16(pa_hi, pc_hi);
    let mask_a_hi = vandq_u16(cond_pa_le_pb_hi, cond_pa_le_pc_hi);
    let cond_pb_le_pc_hi = vcleq_s16(pb_hi, pc_hi);

    let res_bc_hi = vreinterpretq_s16_u16(vbslq_u16(
        cond_pb_le_pc_hi,
        vreinterpretq_u16_s16(vb_hi),
        vreinterpretq_u16_s16(vc_hi),
    ));
    let res_hi = vreinterpretq_s16_u16(vbslq_u16(
        mask_a_hi,
        vreinterpretq_u16_s16(va_hi),
        vreinterpretq_u16_s16(res_bc_hi),
    ));

    let packed_lo = vqmovun_s16(res_lo);
    let packed_hi = vqmovun_s16(res_hi);

    vcombine_u8(packed_lo, packed_hi)
}

/// **PNG 스타일의 Paeth 필터를 이미지 바이트 배열 전체에 적용(인코딩)합니다.**
pub fn apply_png_filter(data: &mut [u8]) {
    let n = data.len();
    if n == 0 {
        return;
    }

    let width = 2048;
    let orig = data.to_vec();
    let _simd_enabled = crate::ENABLE_SIMD.load(std::sync::atomic::Ordering::Relaxed);

    // Width가 2048의 배수이고 SIMD가 활성화된 경우 줄별 병렬화 가속 적용
    if _simd_enabled && n % width == 0 {
        let rows = n / width;

        for r in 0..rows {
            let row_start = r * width;

            if r == 0 {
                // 첫 행: 위쪽이 0이므로 순수 Delta (a 만 사용)
                for col in 0..4 {
                    data[row_start + col] = orig[row_start + col];
                }
                for col in 4..width {
                    data[row_start + col] =
                        orig[row_start + col].wrapping_sub(orig[row_start + col - 4]);
                }
                continue;
            }

            // 두 번째 행부터: 처음 4열은 left=0, upleft=0이므로 paeth_predictor(0, up, 0) = up
            for col in 0..4 {
                let up = orig[row_start + col - width];
                data[row_start + col] = orig[row_start + col].wrapping_sub(up);
            }

            let mut col = 4;

            // x86_64 AVX2 가속 (32바이트씩 병렬 처리)
            #[cfg(target_arch = "x86_64")]
            {
                if is_x86_feature_detected!("avx2") {
                    while col + 31 < width {
                        let idx = row_start + col;
                        unsafe {
                            use std::arch::x86_64::*;
                            let curr_v = _mm256_loadu_si256(orig[idx..].as_ptr() as *const __m256i);
                            let left_v =
                                _mm256_loadu_si256(orig[idx - 4..].as_ptr() as *const __m256i);
                            let up_v =
                                _mm256_loadu_si256(orig[idx - width..].as_ptr() as *const __m256i);
                            let upleft_v = _mm256_loadu_si256(
                                orig[idx - width - 4..].as_ptr() as *const __m256i
                            );

                            let pred_v = paeth_predictor_avx2(left_v, up_v, upleft_v);
                            let res_v = _mm256_sub_epi8(curr_v, pred_v);

                            _mm256_storeu_si256(data[idx..].as_mut_ptr() as *mut __m256i, res_v);
                        }
                        col += 32;
                    }
                }
            }

            // ARM64 NEON 가속 (16바이트씩 병렬 처리)
            #[cfg(target_arch = "aarch64")]
            {
                while col + 15 < width {
                    let idx = row_start + col;
                    unsafe {
                        use std::arch::aarch64::*;
                        let curr_v = vld1q_u8(orig[idx..].as_ptr());
                        let left_v = vld1q_u8(orig[idx - 4..].as_ptr());
                        let up_v = vld1q_u8(orig[idx - width..].as_ptr());
                        let upleft_v = vld1q_u8(orig[idx - width - 4..].as_ptr());

                        let pred_v = paeth_predictor_neon(left_v, up_v, upleft_v);
                        let res_v = vsubq_u8(curr_v, pred_v);

                        vst1q_u8(data[idx..].as_mut_ptr(), res_v);
                    }
                    col += 16;
                }
            }

            // 나머지 열에 대한 순차 처리 루프
            for c in col..width {
                let idx = row_start + c;
                let left = orig[idx - 4];
                let up = orig[idx - width];
                let upleft = orig[idx - width - 4];
                data[idx] = orig[idx].wrapping_sub(paeth_predictor(left, up, upleft));
            }
        }
        return;
    }

    // 순차 Fallback 처리
    for i in 0..n {
        let left = if i >= 4 && (i % width) >= 4 {
            orig[i - 4]
        } else {
            0
        };
        let up = if i >= width { orig[i - width] } else { 0 };
        let upleft = if i >= width + 4 && (i % width) >= 4 {
            orig[i - width - 4]
        } else {
            0
        };
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
        let left = if i >= 4 && (i % width) >= 4 {
            data[i - 4]
        } else {
            0
        };
        let up = if i >= width { data[i - width] } else { 0 };
        let upleft = if i >= width + 4 && (i % width) >= 4 {
            data[i - width - 4]
        } else {
            0
        };

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
        let b0 = data[2 * i]; // 하위 8비트
        let b1 = data[2 * i + 1]; // 상위 8비트

        // `i16::from_le_bytes`: 2바이트 배열 `[u8; 2]`을 입력받아 정밀 16비트 부호 있는 음성 정수로 재합성합니다.
        samples[i] = i16::from_le_bytes([b0, b1]);
    }

    // 필터링 도중 앞선 잔차 결과가 연쇄 왜곡을 주지 않도록 오디오 데이터 사본을 확보합니다.
    // `.clone()`: 깊은 복사(Deep Copy)를 통해 완전히 독립된 별개의 벡터 인스턴스를 하나 복제합니다.
    let orig = samples.clone();

    #[allow(unused_mut)]
    let mut i = 2;

    let _simd_enabled = crate::ENABLE_SIMD.load(std::sync::atomic::Ordering::Relaxed);

    // x86_64 하드웨어 가속 (AVX2 + SSE2)
    #[cfg(target_arch = "x86_64")]
    {
        if _simd_enabled {
            // AVX2 가속 (16개 샘플씩 처리)
            if is_x86_feature_detected!("avx2") && i + 15 < n {
                while i + 15 < n {
                    unsafe {
                        use std::arch::x86_64::*;
                        let val_i = _mm256_loadu_si256(orig[i..].as_ptr() as *const __m256i);
                        let val_prev1 =
                            _mm256_loadu_si256(orig[i - 1..].as_ptr() as *const __m256i);
                        let val_prev2 =
                            _mm256_loadu_si256(orig[i - 2..].as_ptr() as *const __m256i);

                        let two_prev1 = _mm256_add_epi16(val_prev1, val_prev1);
                        let pred = _mm256_sub_epi16(two_prev1, val_prev2);
                        let res = _mm256_sub_epi16(val_i, pred);

                        _mm256_storeu_si256(samples[i..].as_mut_ptr() as *mut __m256i, res);
                    }
                    i += 16;
                }
            }

            // SSE2 fallback (8개 샘플씩 처리)
            if is_x86_feature_detected!("sse2") && i + 7 < n {
                while i + 7 < n {
                    unsafe {
                        use std::arch::x86_64::*;
                        let val_i = _mm_loadu_si128(orig[i..].as_ptr() as *const __m128i);
                        let val_prev1 = _mm_loadu_si128(orig[i - 1..].as_ptr() as *const __m128i);
                        let val_prev2 = _mm_loadu_si128(orig[i - 2..].as_ptr() as *const __m128i);

                        let two_prev1 = _mm_add_epi16(val_prev1, val_prev1);
                        let pred = _mm_sub_epi16(two_prev1, val_prev2);
                        let res = _mm_sub_epi16(val_i, pred);

                        _mm_storeu_si128(samples[i..].as_mut_ptr() as *mut __m128i, res);
                    }
                    i += 8;
                }
            }
        }
    }

    // ARM64 NEON 하드웨어 가속 (8개 샘플씩 처리)
    #[cfg(target_arch = "aarch64")]
    {
        if _simd_enabled {
            while i + 7 < n {
                unsafe {
                    use std::arch::aarch64::*;
                    let val_i = vld1q_s16(orig[i..].as_ptr());
                    let val_prev1 = vld1q_s16(orig[i - 1..].as_ptr());
                    let val_prev2 = vld1q_s16(orig[i - 2..].as_ptr());

                    let two_prev1 = vaddq_s16(val_prev1, val_prev1);
                    let pred = vsubq_s16(two_prev1, val_prev2);
                    let res = vsubq_s16(val_i, pred);

                    vst1q_s16(samples[i..].as_mut_ptr(), res);
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

/// **BWT 연산을 위한 Suffix Array (접미사 배열) 생성 (O(N log N) Counting-Sort Radix Doubling - 순환 교대 정렬)**
fn suffix_array(s: &[u8]) -> Vec<usize> {
    let n = s.len();
    if n == 0 {
        return Vec::new();
    }
    let mut sa: Vec<usize> = (0..n).collect();
    let mut rank: Vec<usize> = s.iter().map(|&x| x as usize).collect();
    let mut k = 1;
    
    let mut sa_temp = vec![0; n];
    let mut sa_out = vec![0; n];
    let mut new_rank = vec![0; n];
    let max_val = n.max(256) + 1;
    let mut count = vec![0; max_val];
    
    while k < n {
        // 1. Sort by secondary key rank[(i + k) % n]
        count.fill(0);
        for i in 0..n {
            count[rank[(i + k) % n]] += 1;
        }
        for i in 1..max_val {
            count[i] += count[i - 1];
        }
        for i in (0..n).rev() {
            let idx = sa[i];
            let next_pos = (idx + k) % n;
            let key = rank[next_pos];
            count[key] -= 1;
            sa_temp[count[key]] = idx;
        }
        
        // 2. Sort by primary key rank[i]
        count.fill(0);
        for i in 0..n {
            count[rank[sa_temp[i]]] += 1;
        }
        for i in 1..max_val {
            count[i] += count[i - 1];
        }
        for i in (0..n).rev() {
            let idx = sa_temp[i];
            let key = rank[idx];
            count[key] -= 1;
            sa_out[count[key]] = idx;
        }
        
        sa.copy_from_slice(&sa_out);
        
        // Recompute ranks
        new_rank[sa[0]] = 0;
        let mut unique_ranks = 1;
        for i in 1..n {
            let prev = sa[i - 1];
            let curr = sa[i];
            let same = rank[prev] == rank[curr]
                && rank[(prev + k) % n] == rank[(curr + k) % n];
            if !same {
                unique_ranks += 1;
            }
            new_rank[curr] = unique_ranks - 1;
        }
        rank.copy_from_slice(&new_rank);
        
        if unique_ranks == n {
            break;
        }
        k *= 2;
    }
    sa
}

/// **BWT (Burrows-Wheeler Transform) 인코딩**
pub fn apply_bwt(s: &[u8]) -> (Vec<u8>, usize) {
    let n = s.len();
    if n == 0 {
        return (Vec::new(), 0);
    }
    let sa = suffix_array(s);
    let mut l = Vec::with_capacity(n);
    let mut primary_index = 0;
    for i in 0..n {
        let idx = sa[i];
        l.push(s[(idx + n - 1) % n]);
        if idx == 0 {
            primary_index = i;
        }
    }
    (l, primary_index)
}

/// **BWT (Burrows-Wheeler Transform) 디코딩**
pub fn inverse_bwt(l: &[u8], primary_index: usize) -> Vec<u8> {
    let n = l.len();
    if n == 0 {
        return Vec::new();
    }
    let mut count = [0; 256];
    for &c in l {
        count[c as usize] += 1;
    }
    let mut sum = 0;
    let mut head = [0; 256];
    for i in 0..256 {
        head[i] = sum;
        sum += count[i];
    }
    let mut next = vec![0; n];
    for i in 0..n {
        let c = l[i] as usize;
        next[head[c]] = i;
        head[c] += 1;
    }
    let mut s = vec![0; n];
    let mut curr = next[primary_index];
    for i in 0..n {
        s[i] = l[curr];
        curr = next[curr];
    }
    s
}

/// **MTF (Move-To-Front) 인코딩**
pub fn apply_mtf(data: &mut [u8]) {
    let mut list = [0u8; 256];
    for i in 0..256 {
        list[i] = i as u8;
    }
    for val in data.iter_mut() {
        let target = *val;
        let mut idx = 0;
        for i in 0..256 {
            if list[i] == target {
                idx = i;
                break;
            }
        }
        *val = idx as u8;
        for i in (1..=idx).rev() {
            list[i] = list[i - 1];
        }
        list[0] = target;
    }
}

/// **MTF (Move-To-Front) 디코딩**
pub fn inverse_mtf(data: &mut [u8]) {
    let mut list = [0u8; 256];
    for i in 0..256 {
        list[i] = i as u8;
    }
    for val in data.iter_mut() {
        let idx = *val as usize;
        let target = list[idx];
        *val = target;
        for i in (1..=idx).rev() {
            list[i] = list[i - 1];
        }
        list[0] = target;
    }
}

/// **BWT + MTF 통합 전처리 필터 적용**
/// - 구조: [4바이트 인덱스] + [MTF(BWT(원본 데이터))]
pub fn apply_bwt_filter(data: &mut Vec<u8>) {
    let n = data.len();
    if n == 0 {
        return;
    }
    let (bwt_output, primary_index) = apply_bwt(data);
    let mut mtf_output = bwt_output;
    apply_mtf(&mut mtf_output);

    data.resize(n + 4, 0);
    let index_bytes = (primary_index as u32).to_le_bytes();
    data[0..4].copy_from_slice(&index_bytes);
    data[4..].copy_from_slice(&mtf_output);
}

/// **BWT + MTF 통합 전처리 필터 해제**
pub fn inverse_bwt_filter(data: &mut Vec<u8>) {
    let n = data.len();
    if n < 4 {
        return;
    }
    let mut index_bytes = [0u8; 4];
    index_bytes.copy_from_slice(&data[0..4]);
    let primary_index = u32::from_le_bytes(index_bytes) as usize;

    let mtf_payload = &mut data[4..];
    inverse_mtf(mtf_payload);
    let restored = inverse_bwt(mtf_payload, primary_index);

    let orig_len = restored.len();
    data[0..orig_len].copy_from_slice(&restored);
    data.truncate(orig_len);
}


