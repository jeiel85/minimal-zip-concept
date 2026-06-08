# MZC Roadmap

This roadmap reflects the current post-0.12.0 state of MZC. Earlier goals
(MZC1 RLE, MZC2 dictionary compression, MZC3 LZ77, MZC4 dynamic Huffman,
MZC5 bit packing, MZC6 tANS/shared dictionaries, and MZC7 context mixing)
are now implemented research milestones rather than future work.

## Completed Milestones

### MZC1 to MZC7 Core Codec Evolution

- MZC1 RLE fixed-header format with SHA-256 verification.
- MZC2 dictionary hybrid compression and backward-compatible parsing.
- MZC3 LZ77 backreferences.
- MZC4 dynamic Huffman coding.
- MZC5 bit-packed stream format with BCJ, Delta, and safety checks.
- MZC6 tANS, hash-chain LZ77, and shared dictionary training.
- MZC7 context-mixing range coder with PNG Paeth and LPC filters.

### Desktop, WASM, and Archive Layer

- CLI commands for compression, decompression, inspection, training, inflate,
  benchmarking, SFX packaging, and context-menu registration.
- egui desktop GUI with dashboard, folder tree, drag-and-drop, entropy plot,
  update checking, and archive visualization.
- WASM browser demo and interactive LZ77 visualizer.
- MZAR archive support with solid/non-solid modes, deduplication, parallel
  compression/extraction, AES-256 password encryption, and CRC32/SHA-256
  checksum paths.
- Self-extracting executable payload support.

## Current Priority: 0.12.x Stabilization

Purpose:

Turn the broad 0.12.0 feature set into a predictable release line.

Focus areas:

- Keep the full codec and archive roundtrip suite green.
- Keep malformed-input tests panic-free.
- Keep release workflow assets aligned with the Cargo package version.
- Keep README, changelog, test plan, and release guide in sync with the actual
  feature set.
- Track and remove build warnings that could become future Cargo or Rust errors.

Completion criteria:

- `cargo test --lib` succeeds.
- Every integration test under `tests/` succeeds.
- `cargo build --release` succeeds.
- `cargo rustc --lib --target wasm32-unknown-unknown --release --crate-type cdylib`
  produces `target/wasm32-unknown-unknown/release/mzc.wasm`.
- `cargo run -- --version` reports the expected Cargo package version.
- A sample compress/decompress/inspect flow restores byte-identical data.

## Next Candidate Goals

### Goal A: Release Verification Automation

Package the manual verification checklist into a repeatable local script or CI
job that runs the same commands before every tag.

Candidate checks:

- Sequential Rust test matrix.
- Release build.
- CLI smoke test with sample files.
- Archive solid/non-solid smoke tests.
- WASM build check.
- Installer metadata/version check.

### Goal B: Archive Format Hardening

Continue improving MZAR safety and debuggability.

Candidate work:

- More corruption tests for entry tables, reference entries, and encrypted
  archive metadata.
- Better inspect output for solid vs non-solid archives.
- Recovery-mode documentation with realistic damaged-archive examples.

### Goal C: Performance Baseline Refresh

Re-run benchmark tables after each substantial codec change and separate
research claims from current measured results.

Candidate work:

- Criterion baseline snapshots for MZC2 through MZC7.
- Compression-ratio fixtures for text, binary, image-like, and audio-like data.
- README benchmark table refresh only from reproducible local outputs.

### Goal D: Public Distribution Polish

Make the user-facing install/update path boring and reliable.

Candidate work:

- Keep GitHub release assets, installer metadata, `latest_version.json`, and
  GUI update checks version-aligned.
- Verify context-menu registration on a clean Windows environment.
- Keep Homebrew/Scoop manifests current after release tags.
