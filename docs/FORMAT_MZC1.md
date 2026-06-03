# MZC1 File Format Specification

## 1. Overview

MZC1 is the first version of the Minimal Zip Concept file format.

It stores compressed data using simple RLE blocks and includes metadata for safe decompression and verification.

## 2. File Layout

```text
[MZC1 Header]
[Payload Blocks]
```

## 3. Header Layout

| Field | Size | Type | Description |
|---|---:|---|---|
| Magic Header | 4 bytes | ASCII | Must be `MZC1` |
| Version | 1 byte | u8 | Must be `0x01` |
| Algorithm Type | 1 byte | u8 | `0x01` means RLE |
| Original Size | 8 bytes | u64 little-endian | Original file size in bytes |
| Payload Size | 8 bytes | u64 little-endian | Compressed payload size in bytes |
| Original SHA-256 | 32 bytes | bytes | SHA-256 hash of the original file |

Total fixed header size:

```text
4 + 1 + 1 + 8 + 8 + 32 = 54 bytes
```

## 4. Payload Structure

The payload is a sequence of blocks.

MZC1 has two block types:

| Block Type | Name | Meaning |
|---:|---|---|
| `0x00` | Literal Block | Stores uncompressed bytes |
| `0x01` | Run Block | Stores repeated bytes |

## 5. Literal Block

Used for data that is not worth compressing.

```text
Type      1 byte   0x00
Length    2 bytes  u16 little-endian
Data      N bytes
```

Example:

```text
0x00 0x05 0x00 H e l l o
```

Meaning:

```text
Literal block of length 5: Hello
```

## 6. Run Block

Used when the same byte repeats multiple times.

```text
Type      1 byte   0x01
Count     2 bytes  u16 little-endian
Value     1 byte
```

Example:

```text
0x01 0x0A 0x00 0x41
```

Meaning:

```text
Byte 0x41 is repeated 10 times.
```

## 7. Compression Rule

MZC1 uses a simple rule:

```text
If the same byte repeats 4 or more times, store it as a Run Block.
Otherwise, store bytes in Literal Blocks.
```

## 8. Maximum Block Length

Both Literal Block length and Run Block count use `u16`.

Maximum value:

```text
65535
```

If a run or literal sequence is longer than 65535 bytes, split it into multiple blocks.

## 9. Verification Rule

During decompression:

1. Read header.
2. Decode payload blocks.
3. Verify restored size equals `Original Size`.
4. Compute SHA-256 of restored data.
5. Compare it with `Original SHA-256` from the header.

Success condition:

```text
SHA-256(restored data) == Original SHA-256
```

If the hash does not match, decompression must fail with a clear error.

## 10. Invalid File Cases

The decoder must reject:

- Wrong magic header
- Unsupported version
- Unsupported algorithm type
- Unknown block type
- Truncated block
- Payload size mismatch
- Restored size mismatch
- SHA-256 mismatch
