# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.0] - 2026-06-05

### Added
- **Solid vs Non-Solid Compression Mode** — support for solid archiving (concatenating all file contents then compressing as a single block, default) and non-solid archiving (compressing each file entry individually then bundling them into an uncompressed MZAR container).
- **Parallel File Compression** — Rayon-based parallel compression for non-solid mode file entries, yielding massive performance gains on multi-core CPUs.
- **GUI Drag-and-Drop** — direct drag-and-drop file/directory receiver support in the egui desktop interface.
- **Auto-detected Decompression** — transparent decompressing/extraction of both solid and non-solid archives without requiring special CLI or GUI flags, using magic-byte signature parsing.
- **Deduplication Filter** — skipped duplicate file contents in MZAR archives using reference entries mapping back to the first occurrence.
- **Parallel Extraction** — Rayon-parallelized file write/extraction pipeline, with subsequent sequential duplicate copying.
- **Shannon Entropy Plot** — 2D sliding-window Shannon Entropy visualization rendered in the GUI Dashboard via `egui_plot`.
- **SFX Self-Extracting Executables** — package archives into standalone executable binaries (`mzc sfx <payload> <output.exe>`) with automatic startup payload checking, extraction, and optional decryption.
- **AES-256 Encryption** — password-based security for both archive formats.
- **Interactive GUI TreeMap** — interactive block visualizer with click-to-extract capability.
- **CLI Benchmark Command** — benchmark suite (`mzc bench`) for performance testing.

## [0.11.1] - 2025-06-05

### Added
- **AVX2 / NEON SIMD acceleration** for Context Mixing (CM) range coder — 36–68% speedup across all compression algorithms (`src/cm.rs`)
- **Streaming decompression pipeline** — direct in-place buffer writes for non-BWT chunks, eliminating intermediate allocations (`src/lib.rs`)
- **WASM custom dictionary upload** — web demo now supports `.dict` file uploads via WASM heap allocation (`src/wasm.rs`, `docs/index.html`)
- **Apple Silicon (`aarch64-apple-darwin`)** release target in CI/CD
- **Debian `.deb` package** generation for Linux x86_64 releases
- **WASM build job** in release workflow — `mzc.wasm` is now included as a release asset
- **O(N log N) Radix Suffix Array** for BWT sorting — replaced Manber-Myers sort with zero-allocation radix sort (`src/filters.rs`)
- **egui folder tree browser** — recursive collapsing file/directory tree in desktop GUI (`src/gui.rs`)
- **Web demo benchmark tab** — live browser comparison of MZC1 through MZC7 algorithms (`docs/index.html`)
- **Criterion micro-benchmarks** for BWT and CM components (`benches/compression_bench.rs`)
- README benchmark tables showing SIMD-optimized `cargo bench` results

### Changed
- `CmModel::weights` reshaped to `[[i32; 8]; 8]` with 64-byte alignment for cache-line optimization
- `CmModel::update()` now accepts pre-calculated probabilities to avoid redundant lookups
- Installer version updated from 0.9.0 to 0.11.1 (`installer/setup.iss`)
- Architecture diagrams in both README files updated to include WASM module and SIMD labels
- Benchmark links in README now use relative paths instead of absolute `file://` URIs

### Fixed
- BWT-filtered chunks (`filter_bits == 6`) correctly use temporary buffers due to 4-byte index prefix requirement during streaming decompression

## [0.11.0] - 2025-05-29

### Added
- **MZC7 Context Mixing & Media Filters Spec** — 0-order, 1-order, 2-order context mixing with arithmetic range coder
- **PNG Paeth filter** for image pixel preprocessing
- **LPC audio filter** — 2nd-order linear predictive coding for 16-bit PCM audio
- **GZIP/DEFLATE decoder** — pure Rust RFC 1951/1952 inflate (`src/deflate.rs`)
- Windows 11 modern context menu integration via Sparse Package
- Interactive LZ77 sliding window web visualizer
- tANS state transition animation plot in GUI
- Dictionary trainer wizard in GUI

### Changed
- Upgraded to 56-byte fixed header format with MZC7 bit-packing flags

## [0.10.0] - 2025-05-20

### Added
- **MZC6 tANS Spec** — table-based Asymmetric Numeral Systems entropy coding
- LZ77 hash chain matching ($O(limit)$ scan)
- Global shared dictionary serialization
- Dictionary training CLI command (`train`)
- WASM interactive web demo with GitHub Pages deployment
- crates.io publishing automation in CI

## [0.9.0] - 2025-05-12

### Added
- **MZC5 Bit-Packed Spec** — 2-bit flag stream packing, lazy matching
- BCJ preprocessor for x86 binary call/jump address translation
- Delta preprocessor for audio/image adjacent-sample differencing
- Decompression safety verification (out-of-bounds detection)

## [0.8.0] - 2025-05-05

### Added
- **MZC4 Dynamic Huffman Spec** — canonical dynamic Huffman tree with Tree RLE header compression
- 20–40 byte slim headers replacing 1KB+ static tree headers

## [0.7.0] - 2025-04-28

### Added
- **MZC3 Sliding Window Spec** — LZ77 with 32KB sliding window + static Huffman coding
- Distance/length backreference encoding

## [0.6.0] - 2025-04-20

### Added
- **MZC2 Parallel Dictionary Spec** — Rayon multi-threaded 1MB chunk parallel compression
- Dictionary-based token substitution

## [0.1.0] - 2025-04-10

### Added
- **MZC1 Retro RLE Spec** — basic Run-Length Encoding
- 54-byte fixed binary header
- CLI compress/decompress commands
- SHA-256 integrity verification
