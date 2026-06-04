use mzc::cli::{CompressionMode, EntropyMode};
use mzc::rle::build_dictionary;

fn main() {
    println!("MZC Shared Dictionary Compression Example");
    println!("=========================================\n");

    // 1. Prepare sample training corpus (similar structured logs)
    let log_samples = vec![
        "2026-06-04T11:00:00Z [INFO] User login succeeded from IP: 192.168.1.10. Status: 200",
        "2026-06-04T11:05:00Z [WARN] API Request slow to /v1/users. Duration: 1200ms",
        "2026-06-04T11:10:00Z [INFO] User database connection pool initialized. Status: 200",
        "2026-06-04T11:15:00Z [ERROR] Failed to fetch credentials. Error: Timeout. Status: 500",
        "2026-06-04T11:20:00Z [INFO] User login succeeded from IP: 192.168.1.15. Status: 200",
    ];

    // Combine logs into a training byte array
    let mut training_corpus = Vec::new();
    for sample in &log_samples {
        training_corpus.extend_from_slice(sample.as_bytes());
    }

    // 2. Train and serialize dictionary
    println!("Training shared dictionary on log corpus...");
    let dict = build_dictionary(&training_corpus);
    let dict_bytes = dict.to_bytes();
    println!("Trained Dictionary size: {} bytes", dict_bytes.len());
    println!(
        "Number of extracted dictionary entries: {}\n",
        dict.entries.len()
    );

    // 3. New input log line to compress
    let test_log =
        "2026-06-04T11:25:00Z [INFO] User login succeeded from IP: 192.168.1.20. Status: 200";
    let test_bytes = test_log.as_bytes();
    println!("Test Log Size (Uncompressed): {} bytes", test_bytes.len());

    // 4. Compress using dictionary
    // We will use MZC7 Hybrid + tANS mode
    println!("Compressing test log with dictionary...");
    let compressed_bytes = mzc::compress_bytes_v2_dict(
        test_bytes,
        CompressionMode::Hybrid,
        EntropyMode::Ans,
        6,
        false,
        false,
        false,
        false,
        Some(&dict_bytes),
    );
    println!(
        "Compressed Size (using dictionary): {} bytes",
        compressed_bytes.len()
    );
    let ratio = (compressed_bytes.len() as f64 / test_bytes.len() as f64) * 100.0;
    println!("Compression Ratio: {:.2}%\n", ratio);

    // 5. Decompress using dictionary
    println!("Decompressing test log with dictionary...");
    let decompressed_bytes = mzc::decompress_bytes_v2_dict(&compressed_bytes, Some(&dict_bytes))
        .expect("Decompression with dictionary failed");

    // 6. Verify roundtrip correctness
    let decompressed_log =
        String::from_utf8(decompressed_bytes).expect("Decompressed bytes are not valid UTF-8");

    if decompressed_log == test_log {
        println!("Success! The decompressed log matches the original test log perfectly.");
    } else {
        println!("Error: Decompressed data mismatch!");
    }
}
