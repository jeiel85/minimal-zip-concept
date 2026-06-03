# MZC1 Test Plan

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

## 2. Required Test Cases

### 2.1 Empty File

Input:

```text
0 bytes
```

Expected:

- Compression succeeds.
- Decompression succeeds.
- Restored file is empty.
- SHA-256 matches.

### 2.2 One-Byte File

Input:

```text
A
```

Expected:

- Stored as Literal Block.
- Roundtrip succeeds.

### 2.3 Repeated Bytes

Input:

```text
AAAAAAAAAAAAAAAAAAAA
```

Expected:

- Stored mostly as Run Block.
- Compression ratio should improve.
- Roundtrip succeeds.

### 2.4 Non-Repeated Text

Input:

```text
The quick brown fox jumps over the lazy dog.
```

Expected:

- Stored mostly as Literal Block.
- Compressed file may be larger than original.
- Roundtrip succeeds.

### 2.5 Mixed Text

Input:

```text
AAAAHelloBBBBWorldCCCC
```

Expected:

- Runs and literals are mixed.
- Roundtrip succeeds.

### 2.6 Binary File

Input:

- Arbitrary binary data

Expected:

- Roundtrip succeeds.
- No UTF-8 assumptions.

### 2.7 Long Run Beyond u16

Input:

```text
A repeated more than 65535 times
```

Expected:

- Split into multiple Run Blocks.
- Roundtrip succeeds.

### 2.8 Long Literal Beyond u16

Input:

```text
Non-repeating data longer than 65535 bytes
```

Expected:

- Split into multiple Literal Blocks.
- Roundtrip succeeds.

## 3. Invalid File Tests

### 3.1 Wrong Magic Header

Expected:

- Decoder rejects file.
- Clear error message.

### 3.2 Unsupported Version

Expected:

- Decoder rejects file.

### 3.3 Unsupported Algorithm Type

Expected:

- Decoder rejects file.

### 3.4 Unknown Block Type

Expected:

- Decoder rejects file.

### 3.5 Truncated Payload

Expected:

- Decoder rejects file.

### 3.6 Payload Size Mismatch

Expected:

- Decoder rejects file.

### 3.7 SHA-256 Mismatch

Expected:

- Decoder rejects file.

## 4. Manual Test Commands

```bash
cargo build
cargo test

cargo run -- compress samples/repeated.txt samples/repeated.mzc
cargo run -- inspect samples/repeated.mzc
cargo run -- decompress samples/repeated.mzc samples/repeated.restored.txt
cargo run -- test samples/repeated.txt
```

## 5. Expected Output Example

```text
Original size: 102400 bytes
Compressed size: 38210 bytes
Ratio: 37.31%
Verified: OK
```
