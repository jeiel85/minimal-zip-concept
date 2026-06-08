# MZC Release Verification Plan

## 1. Test Philosophy

Compression tools must prioritize correctness over compression ratio.

The most important test is roundtrip validation:

```text
original -> compress -> decompress -> restored
```

Success condition:

```text
original bytes == restored bytes
```

For 0.12.x releases, verification must cover the codec layer, archive layer,
CLI smoke path, and release build path. A single `cargo test` run is useful,
but the suite is large enough that release checks should be run sequentially so
Cargo file locks and slow property tests are easier to diagnose.

## 2. Required Test Cases

### 2.1 Codec Roundtrip Cases

- Empty input.
- One-byte input.
- Repeated bytes.
- Non-repeated text.
- Mixed repeated/literal text.
- Arbitrary binary data.
- Long runs beyond `u16`.
- Long literal blocks beyond `u16`.
- Dictionary-trained text.
- MZC3/MZC4/MZC5/MZC6/MZC7 algorithm paths.
- PNG, LPC, BWT, BCJ, Delta, tANS, dynamic Huffman, and context-mixing direct
  tests.

Expected:

```text
original -> compress -> decompress -> restored
original bytes == restored bytes
```

### 2.2 Archive Roundtrip Cases

- Solid archive.
- Non-solid archive.
- Empty directory.
- Nested directory.
- Large file.
- Duplicate file contents.
- CRC32 archive configuration.
- Encrypted archive path.
- Recovery mode path.

Expected:

```text
source directory -> archive -> extract -> restored directory
all restored files match source files
unsafe paths are rejected
```

### 2.3 Invalid File and Robustness Cases

- Wrong magic header.
- Unsupported version.
- Unsupported algorithm type.
- Unknown block type.
- Truncated header.
- Truncated payload.
- Payload size mismatch.
- SHA-256 or CRC32 mismatch.
- Corrupt dictionary layout.
- Invalid token index.
- Random byte input.
- Mutated compressed input.

Expected:

- Decoder rejects invalid data with a structured error.
- Decoder does not panic on malformed input.
- Decoder does not allocate based on untrusted oversized lengths without bounds
  checks.

## 3. Release Verification Commands

Run these commands sequentially from the repository root:

```bash
cargo test --lib
cargo test --test roundtrip_tests
cargo test --test archive_tests
cargo test --test format_tests
cargo test --test robustness_tests
cargo test --test advanced_tests
cargo test --test property_tests
cargo build --release
cargo rustc --lib --target wasm32-unknown-unknown --release --crate-type cdylib
cargo run -- --version
```

Expected:

```text
all tests pass
release build succeeds
WASM build produces target/wasm32-unknown-unknown/release/mzc.wasm
reported CLI version matches Cargo.toml
```

## 4. CLI Smoke Test

Use sample files that can be safely overwritten during local verification.

```bash
cargo run -- compress samples/repeated.txt samples/repeated.release-test.mzc
cargo run -- inspect samples/repeated.release-test.mzc
cargo run -- decompress samples/repeated.release-test.mzc samples/repeated.release-test.restored.txt
```

Expected:

```text
samples/repeated.release-test.restored.txt matches samples/repeated.txt
```

PowerShell byte-identity check:

```powershell
if ((Get-FileHash samples\repeated.txt).Hash -ne (Get-FileHash samples\repeated.release-test.restored.txt).Hash) {
    throw "Release smoke roundtrip failed"
}
```

## 5. Archive Smoke Test

```bash
cargo run -- compress samples samples.release-test.mzc
cargo run -- inspect samples.release-test.mzc
cargo run -- decompress samples.release-test.mzc samples.release-test.out
```

Expected:

```text
archive creates successfully
inspect command reports a valid archive
extract command restores the sample tree
```

## 6. Release Asset Checks

Before tagging a release:

- `Cargo.toml` version matches `CHANGELOG.md`.
- Installer metadata matches the Cargo package version.
- GitHub release tag uses the same semantic version with a leading `v`.
- Homebrew and Scoop manifests are updated when publishing package assets.
- WASM demo assets are rebuilt when `src/wasm.rs` or `docs/index.html` changes.
- README benchmark claims are updated only from reproducible benchmark output.

## 7. Expected Output Example

```text
Original size: 102400 bytes
Compressed size: 38210 bytes
Ratio: 37.31%
Verified: OK
```
