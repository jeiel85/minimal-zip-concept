#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Try to decompress arbitrary inputs.
    // The goal is to verify that the decompression pipeline handles corrupted or malformed data gracefully without panicking.
    let _ = mzc::decompress_bytes_v2(data);
});
