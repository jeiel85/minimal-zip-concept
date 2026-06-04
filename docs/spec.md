# MZC (Minimal Zip Concept) Format & Algorithm Specification

MZC (Minimal Zip Concept) is a multi-version, learning-oriented lossless compression format and CLI/GUI suite written in Rust. This document describes the byte-level layout, preprocessor filters, compression blocks, dictionaries, and entropy coders for all MZC specifications (MZC1 through MZC7).

---

## 1. MZC High-Level File Layout

An MZC compressed file consists of three sequential parts:
1. **File Header**: 54 bytes (MZC1) or 56 bytes (MZC2 to MZC7) of metadata.
2. **Global Shared Dictionary (Optional, MZC6+)**: Present if `Dictionary Size > 0` in the header.
3. **Payload Blocks**: A series of compressed chunks. To support high-performance multi-threading, the payload is partitioned into chunks of up to 1,024,000 bytes (1MB) of original data.

### Chunks Structure
Each chunk in the payload has a 12-byte header followed by the chunk's compressed payload:
- `Original Size` (4 bytes, `u32` little-endian)
- `Combined Size` (4 bytes, `u32` little-endian)
- `Compressed Size` (4 bytes, `u32` little-endian)
- `Compressed Chunk Data` ($N$ bytes, where $N$ matches `Compressed Size`)

---

## 2. File Header Layout

The fixed header sits at the very beginning of the file.

| Offset | Size (bytes) | Type | Description |
| :--- | :---: | :---: | :--- |
| `0..4` | 4 | ASCII | Magic Header (`"MZC1"` to `"MZC7"`) |
| `4` | 1 | `u8` | Format Version (`0x01` to `0x07`) |
| `5` | 1 | `u8` | Algorithm & Feature Flags (detailed below) |
| `6..14` | 8 | `u64` | Original Size of the file in bytes (little-endian) |
| `14..22` | 8 | `u64` | Compressed Payload Size in bytes (little-endian) |
| `22..24` | 2 | `u16` | Dictionary Size (little-endian, MZC2+ only, `0` for MZC1) |
| `24..56` (MZC2+)<br>`22..54` (MZC1) | 32 | Bytes | SHA-256 Checksum of the original uncompressed file |

### 2.1. Algorithm & Feature Flags (Offset 5)

The meaning of the 6th byte (Offset 5) changes across versions.

#### MZC1
- Must be `0x01` (RLE compression).

#### MZC2 / MZC3 / MZC4
- `0x01`: RLE Only (No dictionary)
- `0x02`: Dictionary Only
- `0x03`: Hybrid Mode (RLE, Literal, Token blocks)
- `0x04`: LZ77 Mode

#### MZC5 / MZC6
Flags are bitmapped to represent combinations of preprocessors and entropy coders:
- **Lower 4 bits (Core Algorithm)**:
  - `0x01`: RLE
  - `0x02`: Dict
  - `0x03`: Hybrid
  - `0x04`: LZ77
- **Upper 4 bits (Feature Flags)**:
  - `0x10`: Delta Preprocessor Filter enabled
  - `0x20`: BCJ Preprocessor Filter enabled
  - `0x40`: Dynamic Huffman Entropy Coder enabled
  - `0x80`: Table-based Asymmetric Numeral Systems (tANS) Coder enabled

#### MZC7
The byte is divided into bitfields:
- **Bits 0..1 (Core Algorithm)**:
  - `0`: RLE
  - `1`: Dict
  - `2`: Hybrid
  - `3`: LZ77
- **Bits 2..4 (Entropy Coder)**:
  - `0`: None (Raw serialization)
  - `1`: Static Huffman
  - `2`: Canonical Dynamic Huffman
  - `3`: Table-based ANS (tANS)
  - `4`: Context Mixing Range Coder (CM)
- **Bits 5..7 (Filter Mode)**:
  - `0`: None
  - `1`: Delta Filter
  - `2`: BCJ Filter
  - `3`: PNG Paeth Filter
  - `4`: LPC Audio Filter
  - `5`: Delta + BCJ Filters combined

---

## 3. Preprocessor Filters

Filters reduce the entropy of input data prior to compression. Decoders run them in reverse order.

### 3.1. Delta Filter
Replaces each byte (except the first) with the difference between it and the previous byte.
- **Forward**: $D[i] = X[i] \text{ wrapping\_sub } X[i-1]$
- **Inverse**: $X[i] = D[i] \text{ wrapping\_add } X[i-1]$

### 3.2. BCJ Filter (x86 CPU Branches)
Translates relative offsets in x86 `CALL` (`0xE8`) and `JMP` (`0xE9`) instructions to absolute addresses to enhance repeated patterns.
- **Forward**: Add current index + 5 to the relative 32-bit address.
- **Inverse**: Subtract current index + 5 from the absolute 32-bit address.

### 3.3. PNG Paeth Filter
Uses three neighboring bytes: Left ($a$), Up ($b$), and Upper-Left ($c$) to predict the current pixel value $x$.
- **Predictor formula**: $p = a + b - c$
- Let $pa = |p - a|$, $pb = |p - b|$, and $pc = |p - c|$.
- Return $a$ if $pa \le pb$ and $pa \le pc$; else return $b$ if $pb \le pc$; else return $c$.
- Replaces current pixel with $x \text{ wrapping\_sub } \text{Paeth}(a,b,c)$.

### 3.4. LPC Audio Filter (Linear Predictive Coding)
Applies a 2nd-order linear predictor on 16-bit PCM audio samples.
- **Predictor**: $Pred[i] = 2 \cdot Sample[i-1] - Sample[i-2]$
- Replaces each 16-bit sample with the prediction residual: $Residual[i] = Sample[i] \text{ wrapping\_sub } Pred[i]$.

---

## 4. Core Compression Blocks

MZC serializes data as a stream of blocks.

| Block Type | Name | Type Value | Contents |
| :---: | :--- | :---: | :--- |
| `0` | Literal Block | `0x00` | Raw uncompressed bytes |
| `1` | Run Block | `0x01` | A single byte repeated $N$ times |
| `2` | Token Block | `0x02` | Index reference to a dictionary entry |
| `3` | BackRef Block | `0x03` | Sliding window LZ77 match (Distance, Length) |

### 4.1. MZC2 Block Serialization (Byte-Aligned)
- **Literal**: `[0x00] [Len: u16 LE] [Data: N bytes]`
- **Run**: `[0x01] [Count: u16 LE] [Value: u8]`
- **Token**: `[0x02] [Token Index: u16 LE]`
- **BackRef**: `[0x03] [Distance: u16 LE] [Length: u16 LE]`

### 4.2. MZC5 Bit-Packed Serialization
To eliminate the 1-byte overhead per block, block types are packed in pairs of 2 bits (4 block types represented by 0 to 3) in groups of 8 blocks.
- **Layout**: `[Flags: u16 LE (8 blocks * 2 bits)]` + `[Block Payloads for all 8 blocks]`
- The payloads are serialized back-to-back without block type prefixes.

---

## 5. Dictionaries

### 5.1. Dictionary Layout
A dictionary is serialized as:
- `Entry Count` (2 bytes, `u16` little-endian)
- Repeated for each entry:
  - `Entry Length` (1 byte, `u8` representing length $L \le 255$)
  - `Entry Data` ($L$ bytes)

### 5.2. Training (CLI `train` command)
Scans loaded sample files using a sliding window for patterns of length 4 to 16.
- **Score Formula**: $\text{Score} = (L - 3) \cdot F - (2 + L)$
  - $L$ is the pattern length.
  - $F$ is the occurrence frequency.
- The top 256 scoring candidates are saved into a `.dict` file.

---

## 6. Entropy Coders

### 6.1. Canonical Dynamic Huffman Coding
Reduces Huffman tree header size to 20-40 bytes (vs 1KB static headers).
- **Code Length compression**: Run-length compresses the 256 code lengths.
  - A byte with MSB set (`0x80 | run_len`) represents a run of `run_len + 1` zeros.
  - A byte with MSB clear (`(len << 2) | run_len`) represents `run_len + 1` occurrences of length `len`.

### 6.2. Table-Based ANS (tANS)
Finite State Machine entropy coder utilizing fractional bits.
- State size: $L = 1024$ states.
- Normalizes symbol counts to sum to 1024.
- Maps symbols into states using a coprime step multiplier: $\text{step} = (5 \cdot L / 8) + 3 = 643$.
- Compression parses symbols right-to-left, outputting LSB bits to keep the state within boundary limits.

### 6.3. Context Mixing Range Coder (CM)
A high-ratio compression engine combining direct bit probability prediction with arithmetic range coding.
- **Probability Predictors**:
  - **Context 0**: Path of decoded bits inside the current byte (up to 256 states).
  - **Context 1**: Combination of `(last_byte_1 << 8) | bit_path` (65,536 states).
  - **Context 2**: Hash-mapped index from `(last_byte_2 << 16) | (last_byte_1 << 8) | bit_path` (256KB direct-mapped table).
- **Laplace Smoothing**: Prevents 0% or 100% probabilities: $P = \frac{N_0 + 1}{N_0 + N_1 + 2} \times 4096$.
- **Probability Mixer**: Combines the predictions with weights: $P_{final} = \frac{P_0 + 2 \cdot P_1 + 5 \cdot P_2}{8}$.
- **Range Coding**: Splices the range $[0, 2^{32}-1]$ on final probabilities. Renormalizes and handles overflow/carry output bits dynamically.
