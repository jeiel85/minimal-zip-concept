# MZC1 Implementation Plan

## 1. Target Language

Rust

## 2. Dependencies

Recommended dependencies:

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
sha2 = "0.10"
anyhow = "1"
```

## 3. Project Structure

```text
mzc/
├─ Cargo.toml
├─ README.md
├─ docs/
│  ├─ FORMAT_MZC1.md
│  ├─ ROADMAP.md
│  └─ TEST_PLAN.md
├─ samples/
│  ├─ repeated.txt
│  ├─ normal.txt
│  └─ binary_sample.bin
├─ src/
│  ├─ main.rs
│  ├─ cli.rs
│  ├─ format.rs
│  ├─ rle.rs
│  ├─ checksum.rs
│  ├─ inspect.rs
│  └─ error.rs
└─ tests/
   ├─ roundtrip_tests.rs
   └─ format_tests.rs
```

## 4. Module Responsibilities

| File | Responsibility |
|---|---|
| `main.rs` | Program entry point |
| `cli.rs` | CLI argument parsing |
| `format.rs` | Header structure, read/write logic |
| `rle.rs` | RLE compression and decompression |
| `checksum.rs` | SHA-256 calculation |
| `inspect.rs` | MZC1 metadata inspection |
| `error.rs` | Project-specific error definitions |
| `roundtrip_tests.rs` | Compress-decompress validation |
| `format_tests.rs` | Header and invalid file validation |

## 5. Implementation Steps

### Step 1: Create CLI Skeleton

Implement commands:

```bash
mzc compress <input_file> <output_file>
mzc decompress <input_file> <output_file>
mzc test <input_file>
mzc inspect <input_file>
```

### Step 2: Implement Header Model

Create a `MzcHeader` struct.

Suggested fields:

```rust
pub struct MzcHeader {
    pub version: u8,
    pub algorithm_type: u8,
    pub original_size: u64,
    pub payload_size: u64,
    pub original_sha256: [u8; 32],
}
```

### Step 3: Implement RLE Encoding

Rules:

- Consecutive byte count >= 4: Run Block
- Otherwise: Literal Block
- Split blocks at 65535 bytes

### Step 4: Implement RLE Decoding

Rules:

- Read block type
- Decode based on block type
- Reject unknown block type
- Reject truncated data

### Step 5: Implement Compression Flow

Flow:

```text
read input file
calculate SHA-256
encode payload
create header
write header + payload
print statistics
```

### Step 6: Implement Decompression Flow

Flow:

```text
read MZC file
parse header
read payload
validate payload size
decode payload
validate original size
validate SHA-256
write restored file
```

### Step 7: Implement Test Command

Flow:

```text
read input file
compress in memory
decompress in memory
compare SHA-256
print result
```

### Step 8: Implement Inspect Command

Output:

- Magic header
- Version
- Algorithm type
- Original size
- Payload size
- SHA-256
- Estimated compression ratio

## 6. Completion Criteria

The first goal is complete when:

- `cargo build` succeeds.
- `cargo test` succeeds.
- `mzc compress` works.
- `mzc decompress` works.
- `mzc test` verifies roundtrip correctness.
- `mzc inspect` prints metadata.
- Decompressed files are byte-identical to originals.
