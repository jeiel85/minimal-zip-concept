# MZC vs Industry Standards (Gzip / Zstd) Lossless Compression Benchmark Results

This document contains automatic benchmark evaluation results comparing MZC versions (MZC1 through MZC7) with standard Gzip (RFC 1952) and Zstandard (Zstd) compression engines. All datasets are approximately 200KB in size.

## Dataset: Text (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 3.01 ms | 1.33 ms |
| MZC3 (LZ77+Static) | 13679 bytes | 6.84% | 114.86 ms | 2.11 ms |
| MZC5 (LZ77+Dyn+Filters) | 29315 bytes | 14.66% | 130.53 ms | 4.12 ms |
| MZC6 (tANS) | 41045 bytes | 20.52% | 229.32 ms | 2.42 ms |
| MZC7 (Context Mixing) | 17144 bytes | 8.57% | 190.11 ms | 38.20 ms |
| Gzip (flate2 Default) | 6691 bytes | 3.35% | 1.56 ms | 0.53 ms |
| Zstd (Level 3) | 6985 bytes | 3.49% | 0.78 ms | 0.40 ms |

---

## Dataset: Audio (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 2.84 ms | 0.69 ms |
| MZC3 (LZ77+Static) | 167430 bytes | 83.71% | 239.65 ms | 14.04 ms |
| MZC5 (LZ77+Dyn+Filters) | 177903 bytes | 88.95% | 252.63 ms | 18.07 ms |
| MZC6 (tANS) | 199499 bytes | 99.75% | 290.48 ms | 5.66 ms |
| MZC7 (Context Mixing) | 70443 bytes | 35.22% | 937.14 ms | 130.71 ms |
| Gzip (flate2 Default) | 151452 bytes | 75.73% | 8.99 ms | 3.40 ms |
| Zstd (Level 3) | 140119 bytes | 70.06% | 2.34 ms | 0.28 ms |

---

## Dataset: Image (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 2.89 ms | 0.86 ms |
| MZC3 (LZ77+Static) | 11695 bytes | 5.85% | 155.18 ms | 1.43 ms |
| MZC5 (LZ77+Dyn+Filters) | 25033 bytes | 12.52% | 185.30 ms | 6.02 ms |
| MZC6 (tANS) | 74793 bytes | 37.40% | 865.35 ms | 15.30 ms |
| MZC7 (Context Mixing) | 16397 bytes | 8.20% | 455.44 ms | 88.57 ms |
| Gzip (flate2 Default) | 7726 bytes | 3.86% | 4.17 ms | 1.34 ms |
| Zstd (Level 3) | 10033 bytes | 5.02% | 0.64 ms | 0.28 ms |

---

## Dataset: Executable (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 8.09 ms | 2.45 ms |
| MZC3 (LZ77+Static) | 188810 bytes | 94.41% | 793.29 ms | 29.38 ms |
| MZC5 (LZ77+Dyn+Filters) | 198838 bytes | 99.42% | 123.36 ms | 42.11 ms |
| MZC6 (tANS) | 179786 bytes | 89.89% | 604.77 ms | 13.89 ms |
| MZC7 (Context Mixing) | 168968 bytes | 84.48% | 2575.65 ms | 240.89 ms |
| Gzip (flate2 Default) | 165797 bytes | 82.90% | 23.73 ms | 2.18 ms |
| Zstd (Level 3) | 150821 bytes | 75.41% | 2.30 ms | 0.53 ms |

---

