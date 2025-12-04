//! Performance benchmarks for tools module operations

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use std::time::Duration;

// Mock tool execution benchmark
fn mock_tool_execution(iterations: u64, delay_ms: u64) -> Duration {
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        // Simulate some tool work
        std::hint::black_box(iterations);
        // Simulate I/O delay
        std::thread::sleep(Duration::from_millis(delay_ms));
    }

    start.elapsed()
}

fn bench_tool_execution_overhead(c: &mut Criterion) {
    c.bench_function("tool_execution_overhead", |b| {
        b.iter(|| {
            let duration = mock_tool_execution(black_box(10), black_box(0));
            black_box(duration);
        });
    });

    c.bench_function("tool_execution_small_delay", |b| {
        b.iter(|| {
            let duration = mock_tool_execution(black_box(5), black_box(1));
            black_box(duration);
        });
    });
}

fn bench_json_processing(c: &mut Criterion) {
    let tool_args = r#"{
        "command": "ls -la",
        "working_directory": "/home/user",
        "timeout": 30000,
        "environment": {
            "PATH": "/usr/bin:/bin",
            "HOME": "/home/user"
        }
    }"#;

    c.bench_function("tool_args_serialize", |b| {
        b.iter(|| {
            let parsed: serde_json::Value = serde_json::from_str(&black_box(tool_args)).unwrap();
            black_box(parsed);
        });
    });

    c.bench_function("tool_args_deserialize", |b| {
        let value: serde_json::Value = serde_json::from_str(tool_args).unwrap();
        b.iter(|| {
            let json_str = serde_json::to_string(&black_box(&value)).unwrap();
            black_box(json_str);
        });
    });
}

fn bench_large_json_processing(c: &mut Criterion) {
    // Create a large JSON object similar to complex tool arguments
    let mut large_json = serde_json::json!({
        "command": "find /usr -name '*.so'",
        "args": vec!["-type", "f", "-name", "*.so"],
        "options": {
            "recursive": true,
            "follow_symlinks": false,
            "max_depth": 10
        }
    });

    // Add many files to simulate large output
    let mut files = vec![];
    for i in 0..1000 {
        files.push(format!("/usr/lib/lib{}.so", i));
    }
    large_json["results"] = serde_json::Value::Array(
        files.into_iter().map(serde_json::Value::String).collect()
    );

    let large_json_str = serde_json::to_string(&large_json).unwrap();

    c.bench_function("large_json_serialize", |b| {
        b.iter(|| {
            let json_str = serde_json::to_string(&black_box(&large_json)).unwrap();
            black_box(json_str);
        });
    });

    c.bench_function("large_json_deserialize", |b| {
        b.iter(|| {
            let parsed: serde_json::Value = serde_json::from_str(&black_box(&large_json_str)).unwrap();
            black_box(parsed);
        });
    });
}

fn bench_string_processing(c: &mut Criterion) {
    let test_strings = vec![
        "Simple command output",
        "Line 1\nLine 2\nLine 3\nLine 4\nLine 5",
        &"x".repeat(1000), // Long string
        "Special chars: !@#$%^&*()[]{}|\\;:'\",<>?",
        "Unicode text: Hello 世界 🚀 藍色 中文",
    ];

    c.bench_function("string_truncation", |b| {
        b.iter(|| {
            for s in &black_box(&test_strings) {
                let truncated = if s.len() > 100 { &s[..100] } else { s };
                black_box(truncated);
            }
        });
    });

    c.bench_function("string_line_counting", |b| {
        b.iter(|| {
            for s in &black_box(&test_strings) {
                let count = s.lines().count();
                black_box(count);
            }
        });
    });
}

fn bench_path_operations(c: &mut Criterion) {
    let test_paths = vec![
        "/home/user/.arula/config.yaml",
        "/usr/local/bin/rustc",
        "../../../target/debug/arula-cli",
        "C:\\Users\\Test\\AppData\\Local\\Programs",
        "/very/long/path/that/goes/many/deep/directories/down/to/some/file.txt",
    ];

    c.bench_function("path_normalization", |b| {
        b.iter(|| {
            for path in &black_box(&test_paths) {
                let normalized = std::path::Path::new(path).canonicalize().ok();
                black_box(normalized);
            }
        });
    });

    c.bench_function("path_parent_extraction", |b| {
        b.iter(|| {
            for path in &black_box(&test_paths) {
                let parent = std::path::Path::new(path).parent();
                black_box(parent);
            }
        });
    });
}

fn bench_concurrent_operations(c: &mut Criterion) {
    c.bench_function("concurrent_tool_execution", |b| {
        b.iter(|| {
            use std::sync::Arc;
            use std::thread;

            let handles: Vec<_> = (0..4).map(|i| {
                thread::spawn(move || {
                    mock_tool_execution(black_box(i + 5), black_box(1))
                })
            }).collect();

            for handle in handles {
                let _result = handle.join();
                black_box(_result);
            }
        });
    });
}

criterion_group!(
    benches,
    bench_tool_execution_overhead,
    bench_json_processing,
    bench_large_json_processing,
    bench_string_processing,
    bench_path_operations,
    bench_concurrent_operations
);
criterion_main!(benches);