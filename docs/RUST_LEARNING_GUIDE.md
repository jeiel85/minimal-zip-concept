# Rust Learning Guide for MZC

## 1. Why Rust?

Rust is a good language for this project because compression tools require:

- Byte-level processing
- Reliable file I/O
- Memory safety
- Good performance
- Clear error handling
- Cross-platform CLI support

## 2. Concepts to Learn Through This Project

| Rust Concept | Where It Appears |
|---|---|
| `Vec<u8>` | File bytes and payload buffers |
| Slices | Reading byte ranges |
| Structs | Header representation |
| Enums | Block types and CLI commands |
| `Result` | File I/O and decoding errors |
| Pattern matching | Decoding block types |
| Modules | Project structure |
| Crates | `clap`, `sha2`, `anyhow` |
| Tests | Roundtrip and invalid file validation |

## 3. Recommended Learning Order

### Step 1: Basic Cargo Project

Learn:

- `cargo new`
- `cargo build`
- `cargo run`
- `cargo test`

### Step 2: CLI Parsing

Learn:

- `clap`
- subcommands
- path arguments

### Step 3: File I/O

Learn:

- `std::fs::read`
- `std::fs::write`
- `PathBuf`

### Step 4: Byte Encoding

Learn:

- little-endian conversion
- `u64::to_le_bytes()`
- `u64::from_le_bytes()`
- `u16::to_le_bytes()`
- `u16::from_le_bytes()`

### Step 5: Error Handling

Learn:

- `anyhow::Result`
- `anyhow!`
- `?` operator

### Step 6: Testing

Learn:

- unit tests
- integration tests
- temporary files
- roundtrip validation

## 4. Important Rule

Do not optimize too early.

The correct order is:

```text
make it correct
make it tested
make it readable
make it fast later
```
