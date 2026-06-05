# MZC vs Industry Standards (Gzip / Zstd) Lossless Compression Benchmark Results

This document contains automatic benchmark evaluation results comparing MZC versions (MZC1 through MZC7) with standard Gzip (RFC 1952) and Zstandard (Zstd) compression engines. All datasets are approximately 200KB in size.

## Dataset: Text (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 5.41 ms | 1.08 ms |
| MZC3 (LZ77+Static) | 13690 bytes | 6.84% | 163.52 ms | 2.53 ms |
| MZC5 (LZ77+Dyn+Filters) | 29316 bytes | 14.66% | 186.98 ms | 4.71 ms |
| MZC6 (tANS) | 37885 bytes | 18.94% | 192.41 ms | 1.97 ms |
| MZC7 (Context Mixing) | 18730 bytes | 9.37% | 280.18 ms | 56.68 ms |
| Gzip (flate2 Default) | 6691 bytes | 3.35% | 3.14 ms | 0.58 ms |
| Zstd (Level 3) | 6985 bytes | 3.49% | 0.83 ms | 0.31 ms |

---

## Dataset: Audio (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 3.88 ms | 0.92 ms |
| MZC3 (LZ77+Static) | 167441 bytes | 83.72% | 280.01 ms | 18.26 ms |
| MZC5 (LZ77+Dyn+Filters) | 177902 bytes | 88.95% | 306.21 ms | 23.61 ms |
| MZC6 (tANS) | 199564 bytes | 99.78% | 348.20 ms | 6.68 ms |
| MZC7 (Context Mixing) | 70410 bytes | 35.20% | 578.74 ms | 132.56 ms |
| Gzip (flate2 Default) | 151452 bytes | 75.73% | 10.11 ms | 1.59 ms |
| Zstd (Level 3) | 140119 bytes | 70.06% | 2.41 ms | 0.29 ms |

---

## Dataset: Image (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 3.29 ms | 0.77 ms |
| MZC3 (LZ77+Static) | 11703 bytes | 5.85% | 204.15 ms | 2.11 ms |
| MZC5 (LZ77+Dyn+Filters) | 25021 bytes | 12.51% | 292.38 ms | 5.80 ms |
| MZC6 (tANS) | 74807 bytes | 37.40% | 584.21 ms | 3.25 ms |
| MZC7 (Context Mixing) | 16404 bytes | 8.20% | 439.08 ms | 94.29 ms |
| Gzip (flate2 Default) | 7726 bytes | 3.86% | 3.79 ms | 0.86 ms |
| Zstd (Level 3) | 10033 bytes | 5.02% | 1.32 ms | 0.58 ms |

---

## Dataset: Executable (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 5.63 ms | 1.03 ms |
| MZC3 (LZ77+Static) | 188923 bytes | 94.46% | 665.26 ms | 31.96 ms |
| MZC5 (LZ77+Dyn+Filters) | 198838 bytes | 99.42% | 108.98 ms | 40.04 ms |
| MZC6 (tANS) | 179814 bytes | 89.91% | 539.75 ms | 13.93 ms |
| MZC7 (Context Mixing) | 168968 bytes | 84.48% | 878.49 ms | 299.78 ms |
| Gzip (flate2 Default) | 165797 bytes | 82.90% | 45.99 ms | 1.92 ms |
| Zstd (Level 3) | 150821 bytes | 75.41% | 3.80 ms | 0.45 ms |

---

