use crate::error::MzcError;

// 256KB 크기의 직접 매핑(Direct-Mapped) 해시 테이블 크기 정의
const C2_SIZE: usize = 262144;
// 3차 문맥 해시 테이블 크기 정의
const C3_SIZE: usize = 262144;
// Sparse 비트-바이트 문맥 해시 테이블 크기 정의
const CSPARSE_SIZE: usize = 131072;

/// **컨텍스트 믹싱(Context Mixing) 예측 모델 구조체 (Order-3, Bit Context 및 8차 적응형 LMS 탑재, SIMD 가속 정렬)**
pub struct CmModel {
    pub c0_table: Vec<(u8, u8)>,
    pub c1_table: Vec<(u8, u8)>,
    pub c2_table: Vec<(u8, u8)>,
    pub c3_table: Vec<(u8, u8)>,
    pub c_sparse_table: Vec<(u8, u8)>,
    pub weights: [[i32; 8]; 8],
}

impl CmModel {
    /// **CmModel 생성자 함수**
    pub fn new() -> Self {
        Self {
            c0_table: vec![(0, 0); 256],
            c1_table: vec![(0, 0); 65536],
            c2_table: vec![(0, 0); C2_SIZE],
            c3_table: vec![(0, 0); C3_SIZE],
            c_sparse_table: vec![(0, 0); CSPARSE_SIZE],
            weights: [[1024, 1024, 2048, 2048, 2048, 0, 0, 0]; 8],
        }
    }

    /// **현재 문맥 상태를 바탕으로 다음 비트가 '0'일 확률을 예측하며, 가중치 업데이트에 필요한 개별 문맥 확률 리스트를 반환합니다.**
    pub fn get_probability(
        &self,
        ctx_byte: u16,
        prev_byte_1: u8,
        prev_byte_2: u8,
        prev_byte_3: u8,
        bit_idx: usize,
    ) -> (u32, [u32; 8]) {
        // 1. Context 0 (0차 예측 - 비트 경로)
        let idx0 = ctx_byte as usize;
        let (n0_0, n0_1) = self.c0_table[idx0];
        let p0 = ((n0_0 as u32 + 1) * 4096) / (n0_0 as u32 + n0_1 as u32 + 2);

        // 2. Context 1 (1차 문맥 예측 - 직전 1바이트 + 비트 경로)
        let idx1 = ((prev_byte_1 as usize) << 8) | (ctx_byte as usize);
        let (n1_0, n1_1) = self.c1_table[idx1];
        let p1 = ((n1_0 as u32 + 1) * 4096) / (n1_0 as u32 + n1_1 as u32 + 2);

        // 3. Context 2 (2차 문맥 예측 - 직전 2바이트 + 비트 경로 해시)
        let hash_val =
            (((prev_byte_2 as usize) << 16) | ((prev_byte_1 as usize) << 8) | (ctx_byte as usize))
                % C2_SIZE;
        let (n2_0, n2_1) = self.c2_table[hash_val];
        let p2 = ((n2_0 as u32 + 1) * 4096) / (n2_0 as u32 + n2_1 as u32 + 2);

        // 4. Context 3 (3차 문맥 예측 - 직전 3바이트 + 비트 경로 해시)
        let hash_val_3 = (((prev_byte_3 as usize) << 24)
            | ((prev_byte_2 as usize) << 16)
            | ((prev_byte_1 as usize) << 8)
            | (ctx_byte as usize))
            % C3_SIZE;
        let (n3_0, n3_1) = self.c3_table[hash_val_3];
        let p3 = ((n3_0 as u32 + 1) * 4096) / (n3_0 as u32 + n3_1 as u32 + 2);

        // 5. Sparse Context (직전 바이트 XOR 조합 + 비트 경로 해시)
        let hash_sparse = ((((prev_byte_1 as usize) ^ (prev_byte_2 as usize)) << 8)
            | (ctx_byte as usize))
            % CSPARSE_SIZE;
        let (n4_0, n4_1) = self.c_sparse_table[hash_sparse];
        let p4 = ((n4_0 as u32 + 1) * 4096) / (n4_0 as u32 + n4_1 as u32 + 2);

        let p_arr = [p0, p1, p2, p3, p4, 0, 0, 0];

        // 6. 확률 혼합 (LMS Adaptive Mixing) - SIMD 가속 기동
        let (dot_product, sum_w) = self.mix_probabilities(&p_arr, bit_idx);
        let mut p = dot_product / sum_w;

        if p == 0 {
            p = 1;
        } else if p >= 4096 {
            p = 4095;
        }
        (p, p_arr)
    }

    #[inline(always)]
    fn mix_probabilities(&self, p_arr: &[u32; 8], bit_idx: usize) -> (u32, u32) {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2") {
                unsafe {
                    return avx2_impl::get_probability_avx2(&self.weights[bit_idx], p_arr);
                }
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            unsafe {
                return neon_impl::get_probability_neon(&self.weights[bit_idx], p_arr);
            }
        }

        // Fallback scalar
        let w = self.weights[bit_idx];
        let sum_w = (w[0] + w[1] + w[2] + w[3] + w[4]) as u32;
        let dot_product = w[0] as u32 * p_arr[0]
            + w[1] as u32 * p_arr[1]
            + w[2] as u32 * p_arr[2]
            + w[3] as u32 * p_arr[3]
            + w[4] as u32 * p_arr[4];
        (dot_product, sum_w)
    }

    /// **실제 비트 결과를 통해 통계 모델과 LMS 가중치를 적응적으로 동적 업데이트합니다.**
    pub fn update(
        &mut self,
        ctx_byte: u16,
        prev_byte_1: u8,
        prev_byte_2: u8,
        prev_byte_3: u8,
        bit_idx: usize,
        p_est: u32,
        p_arr: &[u32; 8],
        bit: bool,
    ) {
        // 1. 오차 계산 및 적응형 학습속도(learning_shift) 동적 조정
        let target = if !bit { 4096i32 } else { 0i32 };
        let err = target - p_est as i32;
        let err_abs = err.abs();

        let learning_shift = if err_abs > 2500 {
            13
        } else if err_abs > 1200 {
            14
        } else if err_abs > 500 {
            15
        } else {
            16
        };

        // LMS 가중치 업데이트 SIMD 기동
        self.update_weights_simd(p_arr, p_est, err, learning_shift, bit_idx);

        let update_entry = |c: &mut (u8, u8), bit_val: bool| {
            if !bit_val {
                if c.0 < 255 {
                    c.0 += 1;
                }
            } else {
                if c.1 < 255 {
                    c.1 += 1;
                }
            }
            if c.0 as u16 + c.1 as u16 > 120 {
                c.0 = (c.0 >> 1).max(1);
                c.1 = (c.1 >> 1).max(1);
            }
        };

        // 각 문맥 테이블 업데이트
        let idx0 = ctx_byte as usize;
        update_entry(&mut self.c0_table[idx0], bit);

        let idx1 = ((prev_byte_1 as usize) << 8) | (ctx_byte as usize);
        update_entry(&mut self.c1_table[idx1], bit);

        let hash_val =
            (((prev_byte_2 as usize) << 16) | ((prev_byte_1 as usize) << 8) | (ctx_byte as usize))
                % C2_SIZE;
        update_entry(&mut self.c2_table[hash_val], bit);

        let hash_val_3 = (((prev_byte_3 as usize) << 24)
            | ((prev_byte_2 as usize) << 16)
            | ((prev_byte_1 as usize) << 8)
            | (ctx_byte as usize))
            % C3_SIZE;
        update_entry(&mut self.c3_table[hash_val_3], bit);

        let hash_sparse = ((((prev_byte_1 as usize) ^ (prev_byte_2 as usize)) << 8)
            | (ctx_byte as usize))
            % CSPARSE_SIZE;
        update_entry(&mut self.c_sparse_table[hash_sparse], bit);
    }

    #[inline(always)]
    fn update_weights_simd(
        &mut self,
        p_arr: &[u32; 8],
        p_est: u32,
        err: i32,
        learning_shift: u32,
        bit_idx: usize,
    ) {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2") {
                unsafe {
                    avx2_impl::update_weights_avx2(
                        &mut self.weights[bit_idx],
                        p_arr,
                        p_est,
                        err,
                        learning_shift,
                    );
                    return;
                }
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            unsafe {
                neon_impl::update_weights_neon(
                    &mut self.weights[bit_idx],
                    p_arr,
                    p_est,
                    err,
                    learning_shift,
                );
                return;
            }
        }

        // Fallback scalar
        for i in 0..5 {
            let pi_val = p_arr[i] as i32;
            let delta = (err * (pi_val - p_est as i32)) >> learning_shift;
            self.weights[bit_idx][i] = (self.weights[bit_idx][i] + delta).clamp(128, 16384);
        }
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod avx2_impl {
    use std::arch::x86_64::*;

    #[target_feature(enable = "avx2")]
    pub unsafe fn get_probability_avx2(weights: &[i32; 8], p_arr: &[u32; 8]) -> (u32, u32) {
        let val_w = _mm256_loadu_si256(weights.as_ptr() as *const __m256i);
        let val_p = _mm256_loadu_si256(p_arr.as_ptr() as *const __m256i);

        let mul = _mm256_mullo_epi32(val_w, val_p);
        let dot_product = horizontal_sum_avx2(mul);
        let sum_w = horizontal_sum_avx2(val_w);

        (dot_product as u32, sum_w as u32)
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn update_weights_avx2(
        weights: &mut [i32; 8],
        p_arr: &[u32; 8],
        p_est: u32,
        err: i32,
        learning_shift: u32,
    ) {
        let val_w = _mm256_loadu_si256(weights.as_ptr() as *const __m256i);
        let val_p = _mm256_loadu_si256(p_arr.as_ptr() as *const __m256i);

        let val_err = _mm256_set1_epi32(err);
        let val_p_est = _mm256_set1_epi32(p_est as i32);

        let diff = _mm256_sub_epi32(val_p, val_p_est);
        let prod = _mm256_mullo_epi32(val_err, diff);

        let shift = _mm_cvtsi32_si128(learning_shift as i32);
        let delta = _mm256_sra_epi32(prod, shift);

        let mut w_new = _mm256_add_epi32(val_w, delta);

        let min_val = _mm256_set1_epi32(128);
        let max_val = _mm256_set1_epi32(16384);
        w_new = _mm256_max_epi32(w_new, min_val);
        w_new = _mm256_min_epi32(w_new, max_val);

        let mask = _mm256_setr_epi32(-1, -1, -1, -1, -1, 0, 0, 0);
        w_new = _mm256_and_si256(w_new, mask);

        _mm256_storeu_si256(weights.as_mut_ptr() as *mut __m256i, w_new);
    }

    #[inline(always)]
    unsafe fn horizontal_sum_avx2(x: __m256i) -> i32 {
        let low = _mm256_castsi256_si128(x); // [a, b, c, d]
        let high = _mm256_extracti128_si256(x, 1); // [e, f, g, h]
        let sum128 = _mm_add_epi32(low, high); // [a+e, b+f, c+g, d+h]
        let shifted = _mm_shuffle_epi32(sum128, 0x0E); // [c+g, d+h, c+g, d+h]
        let sum64 = _mm_add_epi32(sum128, shifted); // [a+e+c+g, b+f+d+h, ...]
        let shifted2 = _mm_shuffle_epi32(sum64, 0x01); // [b+f+d+h, ...]
        let sum32 = _mm_add_epi32(sum64, shifted2);
        _mm_cvtsi128_si32(sum32)
    }
}

#[cfg(target_arch = "aarch64")]
mod neon_impl {
    use std::arch::aarch64::*;

    #[inline(always)]
    pub unsafe fn get_probability_neon(weights: &[i32; 8], p_arr: &[u32; 8]) -> (u32, u32) {
        let w_low = vld1q_s32(weights.as_ptr());
        let w_high = vld1q_s32(weights.as_ptr().add(4));
        let p_low = vreinterpretq_s32_u32(vld1q_u32(p_arr.as_ptr()));
        let p_high = vreinterpretq_s32_u32(vld1q_u32(p_arr.as_ptr().add(4)));

        let mul_low = vmulq_s32(w_low, p_low);
        let mul_high = vmulq_s32(w_high, p_high);
        let sum_mul = vaddq_s32(mul_low, mul_high);
        let dot_product = vaddvq_s32(sum_mul);

        let sum_w_vec = vaddq_s32(w_low, w_high);
        let sum_w = vaddvq_s32(sum_w_vec);

        (dot_product as u32, sum_w as u32)
    }

    #[inline(always)]
    pub unsafe fn update_weights_neon(
        weights: &mut [i32; 8],
        p_arr: &[u32; 8],
        p_est: u32,
        err: i32,
        learning_shift: u32,
    ) {
        let w_low = vld1q_s32(weights.as_ptr());
        let w_high = vld1q_s32(weights.as_ptr().add(4));
        let p_low = vreinterpretq_s32_u32(vld1q_u32(p_arr.as_ptr()));
        let p_high = vreinterpretq_s32_u32(vld1q_u32(p_arr.as_ptr().add(4)));

        let err_vec = vdupq_n_s32(err);
        let p_est_vec = vdupq_n_s32(p_est as i32);

        let diff_low = vsubq_s32(p_low, p_est_vec);
        let diff_high = vsubq_s32(p_high, p_est_vec);

        let prod_low = vmulq_s32(err_vec, diff_low);
        let prod_high = vmulq_s32(err_vec, diff_high);

        let shift_vec = vdupq_n_s32(-(learning_shift as i32));
        let delta_low = vshlq_s32(prod_low, shift_vec);
        let delta_high = vshlq_s32(prod_high, shift_vec);

        let mut w_low_new = vaddq_s32(w_low, delta_low);
        let mut w_high_new = vaddq_s32(w_high, delta_high);

        let min_vec = vdupq_n_s32(128);
        let max_vec = vdupq_n_s32(16384);

        w_low_new = vmaxq_s32(w_low_new, min_vec);
        w_low_new = vminq_s32(w_low_new, max_vec);

        w_high_new = vmaxq_s32(w_high_new, min_vec);
        w_high_new = vminq_s32(w_high_new, max_vec);

        let mask_high = vld1q_s32([ -1, 0, 0, 0 ].as_ptr());
        w_high_new = vandq_s32(w_high_new, mask_high);

        vst1q_s32(weights.as_mut_ptr(), w_low_new);
        vst1q_s32(weights.as_mut_ptr().add(4), w_high_new);
    }
}

struct RangeEncoder {
    low: u64,
    range: u32,
    cache_size: u64,
    cache: u8,
    out: Vec<u8>,
}

impl RangeEncoder {
    fn new() -> Self {
        Self {
            low: 0,
            range: 0xFFFF_FFFF,
            cache_size: 1,
            cache: 0,
            out: Vec::new(),
        }
    }

    fn encode_bit(&mut self, bit: bool, p: u32) {
        let boundary = ((self.range as u64 * p as u64) >> 12) as u32;
        if !bit {
            self.range = boundary;
        } else {
            self.low += boundary as u64;
            self.range -= boundary;
        }
        while self.range < 0x0100_0000 {
            self.shift_low();
        }
    }

    fn shift_low(&mut self) {
        let next_byte = (self.low >> 24) as u8;
        if next_byte < 0xFF || self.low >= 0x01_0000_0000 {
            let mut c = self.cache;
            c = c.wrapping_add((self.low >> 32) as u8);
            self.out.push(c);
            for _ in 0..self.cache_size - 1 {
                self.out
                    .push(if self.low >= 0x01_0000_0000 { 0 } else { 0xFF });
            }
            self.cache = next_byte;
            self.cache_size = 1;
        } else {
            self.cache_size += 1;
        }
        self.low = (self.low & 0x00FF_FFFF) << 8;
        self.range <<= 8;
    }

    fn finish(&mut self) {
        for _ in 0..5 {
            self.shift_low();
        }
    }
}

struct RangeDecoder<'a> {
    range: u32,
    code: u32,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> RangeDecoder<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, MzcError> {
        if bytes.len() < 5 {
            return Err(MzcError::TruncatedBlock {
                expected: 5,
                found: bytes.len(),
            });
        }
        let mut dec = Self {
            range: 0xFFFF_FFFF,
            code: 0,
            bytes,
            pos: 0,
        };
        for _ in 0..5 {
            let b = if dec.pos < bytes.len() {
                bytes[dec.pos]
            } else {
                0
            };
            dec.code = (dec.code << 8) | b as u32;
            dec.pos += 1;
        }
        Ok(dec)
    }

    fn decode_bit(&mut self, p: u32) -> bool {
        let boundary = ((self.range as u64 * p as u64) >> 12) as u32;
        if self.code < boundary {
            self.range = boundary;
            while self.range < 0x0100_0000 {
                let b = if self.pos < self.bytes.len() {
                    self.bytes[self.pos]
                } else {
                    0
                };
                self.code = (self.code << 8) | b as u32;
                self.range <<= 8;
                self.pos += 1;
            }
            false
        } else {
            self.range -= boundary;
            self.code -= boundary;
            while self.range < 0x0100_0000 {
                let b = if self.pos < self.bytes.len() {
                    self.bytes[self.pos]
                } else {
                    0
                };
                self.code = (self.code << 8) | b as u32;
                self.range <<= 8;
                self.pos += 1;
            }
            true
        }
    }
}

/// **외부에서 호출하는 고성능 Context Mixing 압축의 핵심 진입점**
pub fn cm_compress(data: &[u8]) -> Result<Vec<u8>, MzcError> {
    let mut encoder = RangeEncoder::new();
    let mut model = CmModel::new();

    let mut prev_byte_1 = 0u8;
    let mut prev_byte_2 = 0u8;
    let mut prev_byte_3 = 0u8;

    for &byte in data {
        let mut ctx_byte = 1u16;
        for i in (0..8).rev() {
            let bit = ((byte >> i) & 1) != 0;
            let bit_idx = (7 - i) as usize;

            let (p, p_arr) = model.get_probability(ctx_byte, prev_byte_1, prev_byte_2, prev_byte_3, bit_idx);
            encoder.encode_bit(bit, p);
            model.update(
                ctx_byte,
                prev_byte_1,
                prev_byte_2,
                prev_byte_3,
                bit_idx,
                p,
                &p_arr,
                bit,
            );

            ctx_byte = (ctx_byte << 1) | (bit as u16);
        }
        prev_byte_3 = prev_byte_2;
        prev_byte_2 = prev_byte_1;
        prev_byte_1 = byte;
    }

    encoder.finish();
    Ok(encoder.out)
}

/// **외부에서 호출하는 Context Mixing 압축 바이트 해제 복원 진입점**
pub fn cm_decompress(cm_bytes: &[u8], original_size: usize) -> Result<Vec<u8>, MzcError> {
    if original_size == 0 {
        return Ok(Vec::new());
    }

    let mut decoder = RangeDecoder::new(cm_bytes)?;
    let mut model = CmModel::new();

    let mut prev_byte_1 = 0u8;
    let mut prev_byte_2 = 0u8;
    let mut prev_byte_3 = 0u8;

    let mut out = Vec::with_capacity(original_size);

    for _ in 0..original_size {
        let mut byte = 0u8;
        let mut ctx_byte = 1u16;
        for i in 0..8 {
            let bit_idx = i;

            let (p, p_arr) = model.get_probability(ctx_byte, prev_byte_1, prev_byte_2, prev_byte_3, bit_idx);
            let bit = decoder.decode_bit(p);

            byte = (byte << 1) | (bit as u8);
            model.update(
                ctx_byte,
                prev_byte_1,
                prev_byte_2,
                prev_byte_3,
                bit_idx,
                p,
                &p_arr,
                bit,
            );
            ctx_byte = (ctx_byte << 1) | (bit as u16);
        }
        out.push(byte);

        prev_byte_3 = prev_byte_2;
        prev_byte_2 = prev_byte_1;
        prev_byte_1 = byte;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert_eq!(*input, decompressed.as_slice(), "Failed on input {}", i);
        }
    }
}
