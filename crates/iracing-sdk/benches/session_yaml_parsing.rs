//! Deterministic Criterion benchmarks for the public session parsing entry points.
//!
//! The benchmark uses a checked-in manifest fixture and measures only typed
//! deserialization. Byte extraction, decoding, and sanitation remain internal
//! contracts and are intentionally not exposed solely for benchmarking.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use iracing_sdk::{
    provider::Provider, providers::ibt::IbtProvider, schema::SessionInfo,
    test_utils::load_fixture_manifest,
};

fn fixture_inputs() -> (
    String,
    iracing_sdk::schema::session::types::SanitizedSessionYaml,
) {
    let fixture = load_fixture_manifest()
        .expect("fixture manifest should load")
        .fixtures
        .into_iter()
        .next()
        .expect("fixture manifest should not be empty");
    let decoded = std::fs::read_to_string(
        fixture
            .session_yaml_file()
            .expect("companion session YAML should exist"),
    )
    .expect("companion session YAML should be readable");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime should build");
    let sanitized = runtime.block_on(async {
        let mut provider =
            IbtProvider::open(fixture.fixture_path().expect("IBT fixture should exist"))
                .expect("IBT fixture should open");
        provider
            .session_yaml(0)
            .await
            .expect("session YAML extraction should succeed")
            .expect("fixture should contain session YAML")
    });

    (decoded, sanitized)
}

fn bench_session_yaml_parsing(c: &mut Criterion) {
    let (decoded, sanitized) = fixture_inputs();
    let mut group = c.benchmark_group("session_yaml_parsing");
    group.throughput(Throughput::Bytes(decoded.len() as u64));

    group.bench_function("decoded_string", |b| {
        b.iter(|| SessionInfo::parse(std::hint::black_box(&decoded)).unwrap())
    });
    group.bench_function("sanitized_provider_yaml", |b| {
        b.iter(|| SessionInfo::parse_sanitized(std::hint::black_box(&sanitized)).unwrap())
    });

    group.finish();
}

criterion_group!(benches, bench_session_yaml_parsing);
criterion_main!(benches);
