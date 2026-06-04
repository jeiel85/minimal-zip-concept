use mzc::cli::{CompressionMode, EntropyMode};

fn main() {
    println!("MZC Simple Compression Example");
    println!("==============================\n");

    // 1. Prepare raw input data
    let original_text = "MZC (Minimal Zip Concept) is a lossless compression format. \
                         This is a sample text that will be compressed using the MZC Rust library. \
                         We want to demonstrate how simple it is to integrate MZC into your own projects! \
                         Let's repeat this line: \
                         MZC (Minimal Zip Concept) is a lossless compression format. \
                         This is a sample text that will be compressed using the MZC Rust library.";
    let original_bytes = original_text.as_bytes();
    println!("Original Data Size: {} bytes", original_bytes.len());

    // 2. Perform in-memory compression
    // We will use MZC7 Hybrid mode + tANS entropy coder, with Level 6 search limits
    // and both Delta and BCJ preprocessing filters enabled.
    let mode = CompressionMode::Hybrid;
    let entropy = EntropyMode::Ans;
    let compression_level = 6;
    let use_delta = true;
    let use_bcj = true;
    let use_png = false;
    let use_lpc = false;

    println!("Compressing data...");
    let compressed_bytes = mzc::compress_bytes_v2(
        original_bytes,
        mode,
        entropy,
        compression_level,
        use_delta,
        use_bcj,
        use_png,
        use_lpc,
    );

    println!("Compressed Data Size: {} bytes", compressed_bytes.len());
    let ratio = (compressed_bytes.len() as f64 / original_bytes.len() as f64) * 100.0;
    println!("Compression Ratio: {:.2}%\n", ratio);

    // 3. Perform decompression
    println!("Decompressing data...");
    let decompressed_bytes =
        mzc::decompress_bytes_v2(&compressed_bytes).expect("Decompression failed");

    // 4. Verify roundtrip correctness
    let decompressed_text =
        String::from_utf8(decompressed_bytes).expect("Decompressed bytes are not valid UTF-8");

    if decompressed_text == original_text {
        println!("Success! The decompressed text matches the original perfectly.");
    } else {
        println!("Error: Decompressed data mismatch!");
    }
}
