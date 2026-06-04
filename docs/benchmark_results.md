# MZC vs Industry Standards (Gzip / Zstd) Lossless Compression Benchmark Results

This document contains automatic benchmark evaluation results comparing MZC versions (MZC1 through MZC7) with standard Gzip (RFC 1952) and Zstandard (Zstd) compression engines. All datasets are approximately 200KB in size.

## Dataset: Text (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 4.88 ms | 0.96 ms |
| MZC3 (LZ77+Static) | 13687 bytes | 6.84% | 170.50 ms | 2.66 ms |
| MZC5 (LZ77+Dyn+Filters) | 29326 bytes | 14.66% | 180.80 ms | 3.84 ms |
| MZC6 (tANS) | 39977 bytes | 19.99% | 232.03 ms | 1.90 ms |
| MZC7 (Context Mixing) | 20284 bytes | 10.14% | 292.59 ms | 34.42 ms |
| Gzip (flate2 Default) | 6691 bytes | 3.35% | 1.85 ms | 0.48 ms |
| Zstd (Level 3) | 6985 bytes | 3.49% | 0.76 ms | 0.28 ms |

---

## Dataset: Audio (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 2.76 ms | 0.66 ms |
| MZC3 (LZ77+Static) | 167428 bytes | 83.71% | 368.28 ms | 19.78 ms |
| MZC5 (LZ77+Dyn+Filters) | 177907 bytes | 88.95% | 358.52 ms | 21.18 ms |
| MZC6 (tANS) | 199349 bytes | 99.67% | 377.41 ms | 8.07 ms |
| MZC7 (Context Mixing) | 75801 bytes | 37.90% | 654.44 ms | 94.99 ms |
| Gzip (flate2 Default) | 151452 bytes | 75.73% | 11.50 ms | 1.64 ms |
| Zstd (Level 3) | 140119 bytes | 70.06% | 2.57 ms | 0.36 ms |

---

## Dataset: Image (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 4.09 ms | 0.85 ms |
| MZC3 (LZ77+Static) | 11697 bytes | 5.85% | 301.77 ms | 2.30 ms |
| MZC5 (LZ77+Dyn+Filters) | 25022 bytes | 12.51% | 325.11 ms | 5.28 ms |
| MZC6 (tANS) | 74780 bytes | 37.39% | 480.24 ms | 3.59 ms |
| MZC7 (Context Mixing) | 16374 bytes | 8.19% | 349.16 ms | 40.48 ms |
| Gzip (flate2 Default) | 7726 bytes | 3.86% | 4.36 ms | 0.73 ms |
| Zstd (Level 3) | 10033 bytes | 5.02% | 0.98 ms | 0.56 ms |

---

## Dataset: Executable (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 4.87 ms | 1.10 ms |
| MZC3 (LZ77+Static) | 188742 bytes | 94.37% | 581.59 ms | 20.61 ms |
| MZC5 (LZ77+Dyn+Filters) | 198838 bytes | 99.42% | 86.13 ms | 23.46 ms |
| MZC6 (tANS) | 179698 bytes | 89.85% | 436.76 ms | 7.19 ms |
| MZC7 (Context Mixing) | 169055 bytes | 84.53% | 639.60 ms | 194.43 ms |
| Gzip (flate2 Default) | 165797 bytes | 82.90% | 28.86 ms | 1.76 ms |
| Zstd (Level 3) | 150821 bytes | 75.41% | 3.10 ms | 0.57 ms |

---

