use crate::error::MzcError;
use std::convert::TryInto;

const L_BITS: usize = 10;
const L: usize = 1 << L_BITS; // 1024 states

#[derive(Clone, Copy)]
struct DecoderEntry {
    symbol: u8,
    nb_bits: u8,
    base: u16,
}

pub fn ans_compress(data: &[u8]) -> Result<Vec<u8>, MzcError> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    // 1. Count frequencies
    let mut counts = [0usize; 256];
    for &b in data {
        counts[b as usize] += 1;
    }

    // 2. Normalize frequencies to sum to L (1024)
    let total_count = data.len();
    let mut f = [0u32; 256];
    let mut sum_f = 0;
    let mut active_symbols = Vec::new();

    for s in 0..256 {
        let count = counts[s];
        if count > 0 {
            active_symbols.push(s);
            let freq = ((count as u64 * L as u64) / total_count as u64) as u32;
            let freq = std::cmp::max(freq, 1);
            f[s] = freq;
            sum_f += freq;
        }
    }

    // Adjust sum_f to be exactly L
    if sum_f != L as u32 {
        let diff = L as i32 - sum_f as i32;
        if diff > 0 {
            let mut best_s = active_symbols[0];
            let mut max_c = counts[best_s];
            for &s in &active_symbols {
                if counts[s] > max_c {
                    max_c = counts[s];
                    best_s = s;
                }
            }
            f[best_s] += diff as u32;
        } else if diff < 0 {
            let mut diff = -diff;
            let mut sorted_active = active_symbols.clone();
            sorted_active.sort_by_key(|&s| std::cmp::Reverse(f[s]));
            for &s in &sorted_active {
                if diff == 0 { break; }
                let val = f[s];
                if val > 1 {
                    let sub = std::cmp::min(val - 1, diff as u32);
                    f[s] -= sub;
                    diff -= sub as i32;
                }
            }
        }
    }

    // 3. Build state_map for encoding
    // state_map stores the list of actual states assigned to each symbol s.
    let mut symbol_table = vec![0u8; L];
    let mut pos = 0;
    let step = (5 * L / 8) + 3; // 643 (coprime to 1024)
    for &s in &active_symbols {
        let freq = f[s] as usize;
        for _ in 0..freq {
            symbol_table[pos] = s as u8;
            pos = (pos + step) & (L - 1);
        }
    }

    let mut state_map = vec![0u16; L];
    let mut next_state = [0u16; 256];
    let mut sym_offset = [0usize; 256];

    let mut offset = 0;
    for &s in &active_symbols {
        sym_offset[s] = offset;
        next_state[s] = f[s] as u16;
        offset += f[s] as usize;
    }

    let mut sym_counters = [0usize; 256];
    for x in 0..L {
        let s = symbol_table[x] as usize;
        let idx = sym_offset[s] + sym_counters[s];
        state_map[idx] = (x + L) as u16;
        sym_counters[s] += 1;
    }

    // 4. Encode data from right to left (backward)
    let mut bits = Vec::new();
    let mut x = L as u16; // Initial state

    for &s in data.iter().rev() {
        let s_idx = s as usize;
        let freq = f[s_idx] as u16;

        // Output LSB bits until x < 2 * freq
        while x >= 2 * freq {
            bits.push((x & 1) != 0);
            x >>= 1;
        }

        // State transition
        let idx = sym_offset[s_idx] + (x - freq) as usize;
        x = state_map[idx];
    }

    // 5. Serialize output
    let mut out = Vec::new();
    
    // Header: Number of active symbols
    let active_count = active_symbols.len() as u16;
    out.extend_from_slice(&active_count.to_le_bytes());

    // Symbol frequencies
    for &s in &active_symbols {
        out.push(s as u8);
        let freq = f[s] as u16;
        out.extend_from_slice(&freq.to_le_bytes());
    }

    // Final state x
    out.extend_from_slice(&x.to_le_bytes());

    // Bitstream bytes (pack bits in original order)
    let mut current_byte = 0u8;
    let mut bit_count = 0;
    let mut bit_bytes = Vec::new();
    for &bit in &bits {
        if bit {
            current_byte |= 1 << bit_count;
        }
        bit_count += 1;
        if bit_count == 8 {
            bit_bytes.push(current_byte);
            current_byte = 0;
            bit_count = 0;
        }
    }
    if bit_count > 0 {
        bit_bytes.push(current_byte);
    }

    // Write bitstream size in bits, and then bit_bytes
    let total_bits = bits.len() as u32;
    out.extend_from_slice(&total_bits.to_le_bytes());
    out.extend_from_slice(&bit_bytes);

    Ok(out)
}

pub fn ans_decompress(ans_bytes: &[u8], original_size: usize) -> Result<Vec<u8>, MzcError> {
    if original_size == 0 {
        return Ok(Vec::new());
    }

    let mut pos = 0;
    let n = ans_bytes.len();

    if pos + 2 > n {
        return Err(MzcError::TruncatedBlock { expected: 2, found: n - pos });
    }

    let active_count = u16::from_le_bytes(ans_bytes[pos..pos + 2].try_into().unwrap()) as usize;
    pos += 2;

    let mut f = [0u32; 256];
    let mut active_symbols = Vec::new();
    let mut sum_f = 0;

    for _ in 0..active_count {
        if pos + 3 > n {
            return Err(MzcError::TruncatedBlock { expected: 3, found: n - pos });
        }
        let s = ans_bytes[pos] as usize;
        let freq = u16::from_le_bytes(ans_bytes[pos + 1..pos + 3].try_into().unwrap()) as u32;
        pos += 3;

        f[s] = freq;
        sum_f += freq;
        active_symbols.push(s);
    }

    if sum_f != L as u32 {
        return Err(MzcError::HuffmanError {
            message: format!("tANS frequency sum mismatch: expected {}, found {}", L, sum_f),
        });
    }

    if pos + 2 > n {
        return Err(MzcError::TruncatedBlock { expected: 2, found: n - pos });
    }
    let mut x = u16::from_le_bytes(ans_bytes[pos..pos + 2].try_into().unwrap());
    pos += 2;

    if pos + 4 > n {
        return Err(MzcError::TruncatedBlock { expected: 4, found: n - pos });
    }
    let total_bits = u32::from_le_bytes(ans_bytes[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;

    let bit_bytes_len = (total_bits + 7) / 8;
    if pos + bit_bytes_len > n {
        return Err(MzcError::TruncatedBlock { expected: bit_bytes_len, found: n - pos });
    }
    let bit_bytes = &ans_bytes[pos..pos + bit_bytes_len];

    // Build Decoder Table
    let mut symbol_table = vec![0u8; L];
    let mut table_pos = 0;
    let step = (5 * L / 8) + 3; // 643
    for &s in &active_symbols {
        let freq = f[s] as usize;
        for _ in 0..freq {
            symbol_table[table_pos] = s as u8;
            table_pos = (table_pos + step) & (L - 1);
        }
    }

    let mut next_state = [0u16; 256];
    for &s in &active_symbols {
        next_state[s] = f[s] as u16;
    }

    let mut decoder_table = vec![DecoderEntry { symbol: 0, nb_bits: 0, base: 0 }; L];
    for state_idx in 0..L {
        let s = symbol_table[state_idx];
        let s_idx = s as usize;
        let y = next_state[s_idx];
        next_state[s_idx] += 1;

        // ilog2(y)
        let ilog2_y = 15 - y.leading_zeros() as usize;
        let nb_bits = (L_BITS - ilog2_y) as u8;
        let base = y << nb_bits;

        decoder_table[state_idx] = DecoderEntry {
            symbol: s,
            nb_bits,
            base,
        };
    }

    // Backward Bit Reader
    let mut bit_pos = total_bits;

    let mut read_bit_backward = || -> bool {
        if bit_pos > 0 {
            bit_pos -= 1;
            let byte_idx = bit_pos / 8;
            let bit_idx = bit_pos % 8;
            (bit_bytes[byte_idx] & (1 << bit_idx)) != 0
        } else {
            false
        }
    };

    let mut read_bits = |nb_bits: u8| -> u16 {
        let mut val = 0u16;
        for _ in 0..nb_bits {
            let bit = read_bit_backward();
            val = (val << 1) | (bit as u16);
        }
        val
    };

    // Decode forward (left to right)
    let mut decompressed = Vec::with_capacity(original_size);
    for _ in 0..original_size {
        if x < L as u16 || x >= (2 * L) as u16 {
            return Err(MzcError::HuffmanError {
                message: format!("tANS state out of range: {}", x),
            });
        }

        let entry = decoder_table[(x - L as u16) as usize];
        decompressed.push(entry.symbol);

        let bits = read_bits(entry.nb_bits);
        x = entry.base + bits;
    }

    Ok(decompressed)
}
