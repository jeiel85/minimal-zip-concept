use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use mzc::{compress_bytes_v2, CompressionMode, EntropyMode};

fn bench_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("mzc_compression");
    
    // Sample datasets of different characteristics
    let text_data = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(100); // ~5.6KB
    let repeated_data = vec![0xAB; 5000];
    
    // Benchmark MZC1 (RLE), MZC2 (LZ77), MZC4 (Deflate), MZC5 (tANS), MZC7 (Context Mixing)
    let algs = vec![
        ("MZC1_RLE", CompressionMode::Rle, EntropyMode::None),
        ("MZC2_LZ77", CompressionMode::Lz77, EntropyMode::None),
        ("MZC4_Deflate", CompressionMode::Lz77, EntropyMode::Dynamic),
        ("MZC5_tANS", CompressionMode::Lz77, EntropyMode::Ans),
        ("MZC7_CM", CompressionMode::Lz77, EntropyMode::Cm),
    ];
    
    for (name, mode, entropy) in algs {
        group.bench_with_input(BenchmarkId::new("compress_text", name), &text_data, |b, data| {
            b.iter(|| {
                let _ = compress_bytes_v2(data, mode, entropy, 3, false, false, false, false);
            });
        });
        
        group.bench_with_input(BenchmarkId::new("compress_repeats", name), &repeated_data, |b, data| {
            b.iter(|| {
                let _ = compress_bytes_v2(data, mode, entropy, 3, false, false, false, false);
            });
        });
    }
    
    group.finish();
}

fn bench_micro(c: &mut Criterion) {
    let mut group = c.benchmark_group("mzc_micro");
    let text_data = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(100); // ~5.6KB
    
    group.bench_function("bwt_apply", |b| {
        b.iter(|| {
            let _ = mzc::filters::apply_bwt(&text_data);
        });
    });
    
    group.bench_function("cm_compress", |b| {
        b.iter(|| {
            let _ = mzc::cm::cm_compress(&text_data);
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_compression, bench_micro);
criterion_main!(benches);
