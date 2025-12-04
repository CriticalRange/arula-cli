//! Performance benchmarks for configuration module operations

use criterion::{criterion_group, criterion_main, Criterion};
use arula_cli::config::{Config, AiConfig};
use std::fs;
use std::hint::black_box;
use tempfile::TempDir;

// Helper function to create test config for benchmarks
fn create_test_config() -> Config {
    Config {
        ai: AiConfig {
            provider: "test-provider".to_string(),
            model: "test-model".to_string(),
            api_url: "https://test.api.com".to_string(),
            api_key: "test-key".to_string(),
        },
    }
}

fn bench_config_creation(c: &mut Criterion) {
    c.bench_function("config_creation_basic", |b| {
        b.iter(|| {
            let config = create_test_config();
            black_box(config);
        });
    });

    c.bench_function("config_creation_manual", |b| {
        b.iter(|| {
            let config = Config {
                ai: AiConfig {
                    provider: black_box("test-provider").to_string(),
                    model: black_box("test-model").to_string(),
                    api_url: black_box("https://test.api.com").to_string(),
                    api_key: black_box("test-key").to_string(),
                },
            };
            black_box(config);
        });
    });
}

fn bench_config_serialization(c: &mut Criterion) {
    let config = create_test_config();

    c.bench_function("config_serialize_json", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&black_box(&config)).unwrap();
            black_box(json);
        });
    });

    c.bench_function("config_serialize_yaml", |b| {
        b.iter(|| {
            let yaml = serde_yaml::to_string(&black_box(&config)).unwrap();
            black_box(yaml);
        });
    });
}

fn bench_config_deserialization(c: &mut Criterion) {
    let config = create_test_config();
    let json_str = serde_json::to_string(&config).unwrap();
    let yaml_str = serde_yaml::to_string(&config).unwrap();

    c.bench_function("config_deserialize_json", |b| {
        b.iter(|| {
            let parsed: Config = serde_json::from_str(&black_box(&json_str)).unwrap();
            black_box(parsed);
        });
    });

    c.bench_function("config_deserialize_yaml", |b| {
        b.iter(|| {
            let parsed: Config = serde_yaml::from_str(&black_box(&yaml_str)).unwrap();
            black_box(parsed);
        });
    });
}

fn bench_config_file_operations(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");
    let config = create_test_config();

    // Pre-create the file for benchmarking
    config.save_to_file(&config_path).unwrap();

    c.bench_function("config_save_to_file", |b| {
        b.iter(|| {
            let config = create_test_config();
            config.save_to_file(&black_box(&config_path)).unwrap();
        });
    });

    c.bench_function("config_load_from_file", |b| {
        b.iter(|| {
            let loaded = Config::load_from_file(&black_box(&config_path)).unwrap();
            black_box(loaded);
        });
    });
}

fn bench_config_default_creation(c: &mut Criterion) {
    c.bench_function("config_default", |b| {
        b.iter(|| {
            let config = Config::default();
            black_box(config);
        });
    });
}

fn bench_config_path_generation(c: &mut Criterion) {
    c.bench_function("config_get_config_path", |b| {
        b.iter(|| {
            let path = Config::get_config_path();
            black_box(path);
        });
    });
}

criterion_group!(
    benches,
    bench_config_creation,
    bench_config_serialization,
    bench_config_deserialization,
    bench_config_file_operations,
    bench_config_default_creation,
    bench_config_path_generation
);
criterion_main!(benches);