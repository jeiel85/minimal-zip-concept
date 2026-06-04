use std::fs;
use std::time::Instant;
use mzc::cli::{CompressionMode, EntropyMode};

// LCG pseudo-random generator
fn get_pseudo_random_bytes(seed: &mut u64, len: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(len);
    for _ in 0..len {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        bytes.push((*seed >> 32) as u8);
    }
    bytes
}

fn generate_text_dataset() -> Vec<u8> {
    let log_templates = [
        "2026-06-04T12:00:00Z [INFO] User login succeeded from IP: 192.168.1.",
        "2026-06-04T12:01:15Z [WARN] API Request slow to /v1/search?query=rust. Duration: ",
        "2026-06-04T12:02:30Z [ERROR] Failed to connect to Redis cache database. Connection timed out. Status: 500",
        "2026-06-04T12:03:45Z [INFO] Process completed background task: rebuild_indices. Status: 200",
    ];

    let mut corpus = Vec::new();
    let mut seed = 12345u64;

    while corpus.len() < 200_000 {
        let template = log_templates[((seed % 4) as usize)];
        corpus.extend_from_slice(template.as_bytes());
        
        // Add random values to keep it realistic
        let rand_val = (seed % 1000).to_string();
        corpus.extend_from_slice(rand_val.as_bytes());
        corpus.extend_from_slice(b"\n");
        
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    }
    corpus.truncate(200_000);
    corpus
}

fn generate_audio_dataset() -> Vec<u8> {
    // Generate 16-bit PCM sound wave (approx 200KB = 100,000 samples)
    let num_samples = 100_000;
    let mut data = Vec::with_capacity(num_samples * 2);
    for i in 0..num_samples {
        // Smooth sine wave pattern
        let angle = (i as f64) * 0.05;
        let val = (angle.sin() * 15000.0) as i16;
        let bytes = val.to_le_bytes();
        data.push(bytes[0]);
        data.push(bytes[1]);
    }
    data
}

fn generate_image_dataset() -> Vec<u8> {
    // Generate 2D gradient grid patterns (approx 200KB)
    let width = 500;
    let height = 400;
    let mut data = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let pixel = ((x ^ y) % 256) as u8;
            data.push(pixel);
        }
    }
    data
}

fn generate_executable_dataset() -> Vec<u8> {
    let mut seed = 77777u64;
    let mut data = get_pseudo_random_bytes(&mut seed, 200_000);
    
    // Inject relative jump instructions (0xE8 and 0xE9)
    for i in (0..data.len() - 5).step_by(20) {
        data[i] = if i % 40 == 0 { 0xE8 } else { 0xE9 };
        // relative offset bytes
        data[i + 1] = 0x12;
        data[i + 2] = 0x34;
        data[i + 3] = 0x56;
        data[i + 4] = 0x78;
    }
    data
}

struct BenchResult {
    version: String,
    ratio: f64,
    compressed_size: usize,
    comp_time_ms: f64,
    decomp_time_ms: f64,
}

fn run_bench_on_dataset(
    name: &str,
    data: &[u8],
) -> Vec<BenchResult> {
    let mut results = Vec::new();

    // Define benchmarks to run
    let test_cases = vec![
        ("MZC1 (RLE)", CompressionMode::Rle, EntropyMode::None, 6, false, false, false, false),
        ("MZC3 (LZ77+Static)", CompressionMode::Lz77, EntropyMode::Huffman, 6, false, false, false, false),
        ("MZC5 (LZ77+Dyn+Filters)", CompressionMode::Lz77, EntropyMode::Dynamic, 6, true, true, false, false),
        ("MZC6 (tANS)", CompressionMode::Hybrid, EntropyMode::Ans, 6, false, false, false, false),
        // MZC7 - customized preprocessor filters depending on dataset
        ("MZC7 (Context Mixing)", CompressionMode::Hybrid, EntropyMode::Cm, 6, false, false, name == "Image", name == "Audio"),
    ];

    for (label, mode, entropy, level, delta, bcj, png, lpc) in test_cases {
        // Measure compression
        let start_comp = Instant::now();
        let compressed = mzc::compress_bytes_v2(
            data,
            mode,
            entropy,
            level,
            delta,
            bcj,
            png,
            lpc,
        );
        let comp_duration = start_comp.elapsed().as_secs_f64() * 1000.0;

        // Measure decompression
        let start_decomp = Instant::now();
        let decompressed = mzc::decompress_bytes_v2(&compressed)
            .expect("Decompression failure during benchmarking");
        let decomp_duration = start_decomp.elapsed().as_secs_f64() * 1000.0;

        assert_eq!(data, decompressed.as_slice(), "Decompressed data mismatch!");

        let ratio = (compressed.len() as f64 / data.len() as f64) * 100.0;

        results.push(BenchResult {
            version: label.to_string(),
            ratio,
            compressed_size: compressed.len(),
            comp_time_ms: comp_duration,
            decomp_time_ms: decomp_duration,
        });
    }

    results
}

fn main() {
    println!("MZC Benchmarking Suite");
    println!("======================\n");

    let datasets = vec![
        ("Text", generate_text_dataset()),
        ("Audio", generate_audio_dataset()),
        ("Image", generate_image_dataset()),
        ("Executable", generate_executable_dataset()),
    ];

    let mut md_report = String::new();
    md_report.push_str("# MZC Lossless Compression Benchmark Results\n\n");
    md_report.push_str("This document contains automatic benchmark evaluation results for MZC1 through MZC7 across different data types (Text, Audio, Image, Executable). All datasets are approximately 200KB in size.\n\n");

    for (name, data) in &datasets {
        println!("Benchmarking dataset: {} ({} bytes)...", name, data.len());
        let results = run_bench_on_dataset(name, data);
        
        md_report.push_str(&format!("## Dataset: {} ({} bytes)\n\n", name, data.len()));
        md_report.push_str("| Format & Mode | Compressed Size | Ratio (%) | Comp Time (ms) | Decomp Time (ms) |\n");
        md_report.push_str("| :--- | :---: | :---: | :---: | :---: |\n");

        for res in results {
            md_report.push_str(&format!(
                "| {} | {} bytes | {:.2}% | {:.2} ms | {:.2} ms |\n",
                res.version, res.compressed_size, res.ratio, res.comp_time_ms, res.decomp_time_ms
            ));
        }
        md_report.push_str("\n---\n\n");
    }

    // Save to docs/benchmark_results.md
    let output_path = "docs/benchmark_results.md";
    fs::write(output_path, &md_report).expect("Failed to write benchmark results file");
    println!("\nBenchmark results saved successfully to: {}", output_path);
}
