//! Representative microbenchmarks for low-level [`VarData`] extraction.
//!
//! The benchmark decodes captured scalar and 72-element array variables from a
//! deterministic frame. Exhaustive type coverage, bounds failures, enum
//! conversion, and bitfield behavior belong to correctness tests rather than
//! the performance suite.
//!
//! Schema loading, fixture generation, lookups, and sentinel assertions happen
//! before timing. Use `aggregate-frame-parsing` for complete consumer workloads.
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench var-data-extraction
//! ```

mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use iracing_sdk::{VarData, irsdk::VariableType};
use std::hint::black_box;

fn load_test_data() -> (Vec<u8>, iracing_sdk::VariableSchema) {
    let fixture = support::full_frame_fixture();
    (fixture.data, fixture.schema.as_ref().clone())
}

fn bench_scalar_extraction(c: &mut Criterion) {
    let (data, schema) = load_test_data();
    let mut group = c.benchmark_group("scalar_extraction");

    let session_time = support::require_variable(&schema, "SessionTime", VariableType::Double, 1);
    assert_eq!(f64::from_bytes(&data, session_time).unwrap(), 0.5);
    group.bench_function("f64_session_time", |b| {
        b.iter(|| black_box(f64::from_bytes(black_box(&data), session_time).unwrap()))
    });

    let speed = support::require_variable(&schema, "Speed", VariableType::Float, 1);
    assert_eq!(f32::from_bytes(&data, speed).unwrap(), 0.5);
    group.bench_function("f32_speed", |b| {
        b.iter(|| black_box(f32::from_bytes(black_box(&data), speed).unwrap()))
    });

    let gear = support::require_variable(&schema, "Gear", VariableType::Integer, 1);
    assert_eq!(i32::from_bytes(&data, gear).unwrap(), 1);
    group.bench_function("i32_gear", |b| {
        b.iter(|| black_box(i32::from_bytes(black_box(&data), gear).unwrap()))
    });

    let marker = support::require_variable(&schema, "DriverMarker", VariableType::Boolean, 1);
    assert!(bool::from_bytes(&data, marker).unwrap());
    group.bench_function("bool_driver_marker", |b| {
        b.iter(|| black_box(bool::from_bytes(black_box(&data), marker).unwrap()))
    });

    group.finish();
}

fn bench_array_extraction(c: &mut Criterion) {
    let (data, schema) = load_test_data();
    let mut group = c.benchmark_group("array_extraction");
    group.throughput(Throughput::Elements(72));

    let lap_distance =
        support::require_variable(&schema, "CarIdxLapDistPct", VariableType::Float, 72);
    let values = Vec::<f32>::from_bytes(&data, lap_distance).unwrap();
    assert_eq!((values.len(), values[0], values[71]), (72, 0.5, 71.5));
    group.bench_function(BenchmarkId::new("f32_array", 72), |b| {
        b.iter(|| black_box(Vec::<f32>::from_bytes(black_box(&data), lap_distance).unwrap()))
    });

    let track_surface =
        support::require_variable(&schema, "CarIdxTrackSurface", VariableType::Integer, 72);
    let values = Vec::<i32>::from_bytes(&data, track_surface).unwrap();
    assert_eq!((values.len(), values[0], values[71]), (72, 1, 72));
    group.bench_function(BenchmarkId::new("i32_array", 72), |b| {
        b.iter(|| black_box(Vec::<i32>::from_bytes(black_box(&data), track_surface).unwrap()))
    });

    let pit_road = support::require_variable(&schema, "CarIdxOnPitRoad", VariableType::Boolean, 72);
    let values = Vec::<bool>::from_bytes(&data, pit_road).unwrap();
    assert_eq!(values.len(), 72);
    assert!(values[0]);
    assert!(!values[1]);
    group.bench_function(BenchmarkId::new("bool_array", 72), |b| {
        b.iter(|| black_box(Vec::<bool>::from_bytes(black_box(&data), pit_road).unwrap()))
    });

    group.finish();
}

criterion_group!(benches, bench_scalar_extraction, bench_array_extraction);
criterion_main!(benches);
