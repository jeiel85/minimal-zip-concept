# MZC Lossless Compression Benchmark Results

This document contains automatic benchmark evaluation results for MZC1 through MZC7 across different data types (Text, Audio, Image, Executable). All datasets are approximately 200KB in size.

## Dataset: Text (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 3.56 ms | 0.35 ms |
| MZC3 (LZ77+Static) | 13691 bytes | 6.85% | 143.23 ms | 0.80 ms |
| MZC5 (LZ77+Dyn+Filters) | 14720 bytes | 7.36% | 147.78 ms | 0.98 ms |
| MZC6 (tANS) | 42239 bytes | 21.12% | 165.01 ms | 0.79 ms |
| MZC7 (Context Mixing) | 20915 bytes | 10.46% | 165.66 ms | 8.30 ms |

---

## Dataset: Audio (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 1.45 ms | 0.32 ms |
| MZC3 (LZ77+Static) | 167122 bytes | 83.56% | 987.45 ms | 5.66 ms |
| MZC5 (LZ77+Dyn+Filters) | 176794 bytes | 88.40% | 892.01 ms | 7.19 ms |
| MZC6 (tANS) | 197297 bytes | 98.65% | 2423.22 ms | 8.53 ms |
| MZC7 (Context Mixing) | 76931 bytes | 38.47% | 2006.71 ms | 75.08 ms |

---

## Dataset: Image (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 5.36 ms | 0.88 ms |
| MZC3 (LZ77+Static) | 10821 bytes | 5.41% | 721.42 ms | 2.13 ms |
| MZC5 (LZ77+Dyn+Filters) | 12758 bytes | 6.38% | 728.29 ms | 4.07 ms |
| MZC6 (tANS) | 25329 bytes | 12.66% | 903.87 ms | 2.37 ms |
| MZC7 (Context Mixing) | 17815 bytes | 8.91% | 545.72 ms | 22.54 ms |

---

## Dataset: Executable (200000 bytes)

| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |
| :--- | :---: | :---: | :---: | :---: |
| MZC1 (RLE) | 200080 bytes | 100.04% | 6.32 ms | 1.14 ms |
| MZC3 (LZ77+Static) | 189151 bytes | 94.58% | 3701.55 ms | 19.47 ms |
| MZC5 (LZ77+Dyn+Filters) | 198798 bytes | 99.40% | 3225.10 ms | 27.21 ms |
| MZC6 (tANS) | 181786 bytes | 90.89% | 3588.00 ms | 8.86 ms |
| MZC7 (Context Mixing) | 168881 bytes | 84.44% | 3764.16 ms | 89.54 ms |

---

