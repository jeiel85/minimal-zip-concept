# MZC Roadmap

## Goal 1: MZC1 RLE Compression

Purpose:

Build the first working version of the compression tool.

Features:

- Rust CLI
- Custom MZC1 file format
- RLE compression
- RLE decompression
- SHA-256 verification
- Inspect command
- Test command

Completion criteria:

- `cargo build` succeeds.
- `cargo test` succeeds.
- Compress-decompress roundtrip is byte-perfect.

## Goal 2: Stability and Test Hardening

Purpose:

Make MZC1 safer and more reliable.

Features:

- More roundtrip tests
- Invalid file tests
- Better error messages
- Edge case handling
- Empty file support
- Long run splitting
- Long literal splitting
- README and docs cleanup

Completion criteria:

- All edge cases are covered by tests.
- Invalid files fail safely.
- No panic on malformed input.

## Goal 3: MZC2 Dictionary Compression Plan

Purpose:

Prepare the next format version for text-heavy data.

MZC2 should target files with repeated words, repeated tokens, repeated JSON keys, sermon scripts, Bible study documents, game data, and configuration files.

Possible features:

- Dictionary section
- Token blocks
- Frequent sequence detection
- RLE-only mode
- Dictionary-only mode
- Hybrid mode
- Compression ratio comparison

## Goal 4: MZC3 LZ-Style Experiment

Purpose:

Experiment with sliding-window compression.

Possible features:

- Back-reference blocks
- Window size configuration
- Match length and distance encoding
- Comparison with MZC1 and MZC2

## Goal 5: GUI or App Integration

Possible directions:

- Windows desktop GUI
- Android demo app
- Game data packing tool
- Sermon text archive compressor
- Pixel art asset packer
