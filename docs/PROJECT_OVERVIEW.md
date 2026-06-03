# MZC Project Overview

## 1. Project Name

**MZC**: Minimal Zip Concept

## 2. Purpose

MZC is a learning-oriented custom lossless compression project.

The purpose is not to outperform ZIP, Zstandard, Brotli, LZMA, or other mature compression algorithms. Instead, the purpose is to understand compression concepts by building a small, verifiable, custom file format and CLI tool.

## 3. First Milestone

The first milestone is **MZC1**.

MZC1 uses:

- Rust
- A custom binary file format
- RLE-based compression
- SHA-256 verification
- CLI commands for compression, decompression, testing, and inspection

## 4. CLI Commands

```bash
mzc compress <input_file> <output_file>
mzc decompress <input_file> <output_file>
mzc test <input_file>
mzc inspect <input_file>
```

## 5. Core Principle

Compression is only successful if decompression restores the original file exactly.

```text
original bytes == decompressed bytes
```

The project verifies this using SHA-256.

```text
SHA-256(original) == SHA-256(decompressed)
```

## 6. Design Priorities

Priority order:

1. Correctness
2. Lossless restoration
3. Clear file format
4. Readable Rust code
5. Strong tests
6. Good documentation
7. Performance optimization later

## 7. Rust Learning Goals

This project is designed to teach:

- File I/O
- Byte arrays
- `Vec<u8>`
- Slices
- Structs
- Enums
- Error handling with `Result`
- CLI parsing with `clap`
- SHA-256 hashing with `sha2`
- Modular Rust project structure
- Integration tests
