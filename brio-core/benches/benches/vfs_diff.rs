//! Benchmarks for hash computation in vfs/diff.rs
//!
//! Performance-critical paths:
//! - `compute_hash`: SHA-256 hashing of file contents
//! - `scan_directory`: Directory traversal and metadata collection
//! - `compute_diff`: Comparing two directory snapshots

#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

fn compute_hash(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn bench_compute_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("vfs_diff/compute_hash");

    // Test different file sizes
    let sizes = [1024usize, 8192, 65536, 524_288, 1_048_576, 10_485_760];

    for size in sizes {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test_file.bin");
        let data = vec![0u8; size];
        fs::write(&file_path, &data).unwrap();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{size}_bytes")),
            &size,
            |b, _| b.iter(|| compute_hash(black_box(&file_path))),
        );
    }

    group.finish();
}

fn bench_compute_hash_small_files(c: &mut Criterion) {
    let mut group = c.benchmark_group("vfs_diff/compute_hash_small");

    // Small file sizes (common in source code projects)
    let sizes = [
        ("1kb", 1024usize),
        ("4kb", 4096),
        ("16kb", 16384),
        ("64kb", 65536),
    ];

    for (name, size) in sizes {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        // Simulate source code file with varied content
        let content: String = (0..size)
            .map(|i| char::from(b'a' + u8::try_from(i % 26).unwrap()))
            .collect();
        fs::write(&file_path, content).unwrap();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(name, |b| b.iter(|| compute_hash(black_box(&file_path))));
    }

    group.finish();
}

fn bench_buffer_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("vfs_diff/hash_buffer_size");

    // Benchmark different buffer sizes for hashing
    let file_size = 1024 * 1024; // 1MB
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.bin");
    let data = vec![0u8; file_size];
    fs::write(&file_path, &data).unwrap();

    let buffer_sizes = [1024usize, 4096, 8192, 16384, 65536];

    for buf_size in buffer_sizes {
        group.bench_function(format!("{buf_size}_bytes"), |b| {
            b.iter(|| {
                let mut file = fs::File::open(&file_path).unwrap();
                let mut hasher = Sha256::new();
                let mut buffer = vec![0u8; buf_size];

                loop {
                    let count = file.read(&mut buffer).unwrap();
                    if count == 0 {
                        break;
                    }
                    hasher.update(&buffer[..count]);
                }

                black_box(hex::encode(hasher.finalize()))
            });
        });
    }

    group.finish();
}

fn bench_scan_directory(c: &mut Criterion) {
    let mut group = c.benchmark_group("vfs_diff/scan_directory");

    // Test different directory sizes
    let file_counts = [10usize, 100, 1000];

    for count in file_counts {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        // Create nested directory structure with files
        for i in 0..count {
            let subdir = root.join(format!("dir{}", i % 10));
            fs::create_dir_all(&subdir).unwrap();
            let file_path = subdir.join(format!("file{i}.txt"));
            fs::write(&file_path, format!("content {i}")).unwrap();
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{count}_files")),
            &count,
            |b, _| {
                b.iter(|| {
                    let mut files = Vec::new();
                    for entry in walkdir::WalkDir::new(black_box(root)) {
                        if let Ok(e) = entry
                            && e.file_type().is_file()
                        {
                            files.push(e.path().to_path_buf());
                        }
                    }
                    black_box(files)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_compute_hash,
    bench_compute_hash_small_files,
    bench_buffer_sizes,
    bench_scan_directory
);
criterion_main!(benches);
