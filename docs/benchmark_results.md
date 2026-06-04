# MZC vs Industry Standards (Gzip / Zstd) Lossless Compression Benchmark Results

This document contains automatic benchmark evaluation results comparing MZC versions (MZC1 through MZC7) with standard Gzip (RFC 1952) and Zstandard (Zstd) compression engines. All datasets are approximately 200KB in size.

## Dataset: Text (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 4.88 ms | 0.51 ms |
| MZC3 (LZ77+Static) | 13693 bytes | 6.85% | 207.74 ms | 1.01 ms |
| MZC5 (LZ77+Dyn+Filters) | 14709 bytes | 7.35% | 202.52 ms | 1.43 ms |
| MZC6 (tANS) | 42677 bytes | 21.34% | 234.92 ms | 1.11 ms |
| MZC7 (Context Mixing) | 21225 bytes | 10.61% | 282.73 ms | 16.30 ms |
| Gzip (flate2 Default) | 6691 bytes | 3.35% | 1.08 ms | 0.26 ms |
| Zstd (Level 3) | 6985 bytes | 3.49% | 0.45 ms | 0.14 ms |

---

## Dataset: Audio (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 2.16 ms | 0.47 ms |
| MZC3 (LZ77+Static) | 167107 bytes | 83.55% | 1639.29 ms | 7.69 ms |
| MZC5 (LZ77+Dyn+Filters) | 176740 bytes | 88.37% | 2794.29 ms | 17.04 ms |
| MZC6 (tANS) | 197143 bytes | 98.57% | 4500.85 ms | 10.85 ms |
| MZC7 (Context Mixing) | 76939 bytes | 38.47% | 2234.36 ms | 95.92 ms |
| Gzip (flate2 Default) | 151452 bytes | 75.73% | 16.32 ms | 2.35 ms |
| Zstd (Level 3) | 140119 bytes | 70.06% | 3.18 ms | 0.47 ms |

---

## Dataset: Image (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 4.18 ms | 0.99 ms |
| MZC3 (LZ77+Static) | 10821 bytes | 5.41% | 742.14 ms | 2.19 ms |
| MZC5 (LZ77+Dyn+Filters) | 12760 bytes | 6.38% | 806.16 ms | 2.84 ms |
| MZC6 (tANS) | 25334 bytes | 12.67% | 865.36 ms | 4.19 ms |
| MZC7 (Context Mixing) | 17813 bytes | 8.91% | 597.62 ms | 42.61 ms |
| Gzip (flate2 Default) | 7726 bytes | 3.86% | 5.17 ms | 1.00 ms |
| Zstd (Level 3) | 10033 bytes | 5.02% | 1.60 ms | 0.53 ms |

---

## Dataset: Executable (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 6.41 ms | 1.36 ms |
| MZC3 (LZ77+Static) | 189176 bytes | 94.59% | 3589.88 ms | 22.03 ms |
| MZC5 (LZ77+Dyn+Filters) | 198798 bytes | 99.40% | 3095.76 ms | 25.73 ms |
| MZC6 (tANS) | 181821 bytes | 90.91% | 3608.46 ms | 14.89 ms |
| MZC7 (Context Mixing) | 168920 bytes | 84.46% | 3676.30 ms | 80.94 ms |
| Gzip (flate2 Default) | 165797 bytes | 82.90% | 28.15 ms | 1.63 ms |
| Zstd (Level 3) | 150821 bytes | 75.41% | 4.50 ms | 0.27 ms |

---

