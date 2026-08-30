//! Session YAML preprocessing and typed-deserialization benchmarks.
//!
//! Both cases use the checked-in live session snapshot rather than a synthetic
//! partial document. Fixture I/O, UTF-8 decoding, and correctness checks happen
//! before Criterion starts timing.
//!
//! - `preprocess/live_snapshot` measures the production sanitization pass and
//!   allocation of its owned output.
//! - `deserialize/live_snapshot` measures deserialization of already prepared
//!   YAML into a fresh [`iracing_sdk::SessionInfo`]. It excludes preprocessing
//!   so changes in the two stages can be distinguished.
//!
//! Outputs are passed to [`std::hint::black_box`] and destroyed normally. Byte
//! throughput is the input YAML size, not the size of allocated Rust values.
//! Criterion reports statistically sampled measurements; benchmark results are
//! not correctness thresholds and do not run as part of the unit-test suite.
//!
//! Run this target with:
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench session_parsing
//! ```

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use iracing_sdk::{SessionInfo, benchmarking::preprocess_session_yaml};
use std::{fs, hint::black_box, path::PathBuf};

fn live_session_snapshot() -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-data/live-session-snapshot.yml");
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn validate_snapshot(yaml: &str) {
    let session = SessionInfo::try_from(yaml)
        .unwrap_or_else(|error| panic!("live session benchmark snapshot is invalid: {error}"));

    assert_eq!(session.weekend_info.track_name, "roadamerica full");
    assert_eq!(session.session_info.sessions.len(), 1);
    assert_eq!(
        session
            .driver_info
            .as_ref()
            .and_then(|driver_info| driver_info.drivers.as_ref())
            .map(Vec::len),
        Some(1),
        "live session benchmark snapshot should contain one driver"
    );
}

fn bench_session_parsing(c: &mut Criterion) {
    let raw_yaml = live_session_snapshot();
    let prepared_yaml = preprocess_session_yaml(&raw_yaml)
        .unwrap_or_else(|error| panic!("failed to preprocess benchmark snapshot: {error}"));
    validate_snapshot(&prepared_yaml);

    let mut preprocessing = c.benchmark_group("session_parsing/preprocess");
    preprocessing.throughput(Throughput::Bytes(raw_yaml.len() as u64));
    preprocessing.bench_function("live_snapshot", |b| {
        b.iter(|| {
            let yaml = preprocess_session_yaml(black_box(raw_yaml.as_str()))
                .expect("validated session YAML should remain preprocessable");
            black_box(yaml)
        })
    });
    preprocessing.finish();

    let mut deserialization = c.benchmark_group("session_parsing/deserialize");
    deserialization.throughput(Throughput::Bytes(prepared_yaml.len() as u64));
    deserialization.bench_function("live_snapshot", |b| {
        b.iter(|| {
            let session = SessionInfo::try_from(black_box(prepared_yaml.as_str()))
                .expect("validated session YAML should remain parseable");
            black_box(session)
        })
    });
    deserialization.finish();
}

criterion_group!(benches, bench_session_parsing);
criterion_main!(benches);
