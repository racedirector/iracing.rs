//! Microbenchmarks for low-level [`VarData`] extraction and bitfield operations.
//!
//! # What is being measured
//!
//! The benchmark loads the checked-in live variable-schema capture and creates
//! one deterministic frame before timing. Each group targets one narrow task:
//!
//! - `scalar_extraction` decodes individual `f64`, `f32`, `i32`, and `bool`
//!   values using real captured names, types, counts, and offsets.
//! - `array_extraction` decodes three captured 72-element `CarIdx` arrays into
//!   fresh `Vec<f32>`, `Vec<i32>`, and `Vec<bool>` outputs. Element throughput
//!   includes allocation, decoding, and destruction of each vector.
//! - `bitfield_operations/bitfield_extraction` decodes `SessionFlags`; the
//!   remaining bitfield cases operate on an already decoded [`BitField`].
//! - `bounds_checking` compares successful scalar decoding with deliberately
//!   invalid scalar and array offsets. Invalid cases measure the expected error
//!   path rather than successful parsing throughput.
//!
//! Variable lookup, metadata validation, invalid-metadata construction, and
//! sentinel assertions happen before timing. Timed results are passed to
//! [`std::hint::black_box`] so the compiler must retain the operation.
//!
//! # Reading results
//!
//! These are isolated operations, not whole-frame parsing estimates.
//! Multiplying one result by the schema variable count ignores the schema's
//! type mix, arrays, result collection, allocation, and cache behavior. Use
//! `aggregate_frame_parsing.rs` for complete consumer workloads.
//!
//! Frame acquisition, schema discovery, adapters, subscription delivery,
//! serialization, and application processing are outside this target's scope.
//!
//! Run this target with:
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench var_data_extraction
//! ```

mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use iracing_sdk::types::{BitField, VarData, VariableType};
use std::hint::black_box;

/// Load deterministic full-frame data and variable info for benchmarking.
fn load_test_data() -> (Vec<u8>, iracing_sdk::VariableSchema) {
    let fixture = support::full_frame_fixture();
    (fixture.data, fixture.schema.as_ref().clone())
}

/// Measure successful extraction of representative captured scalar types.
fn bench_scalar_extraction(c: &mut Criterion) {
    let (data, schema) = load_test_data();

    let mut group = c.benchmark_group("scalar_extraction");

    // Benchmark common scalar types with real variables
    let session_time_info =
        support::require_variable(&schema, "SessionTime", VariableType::Double, 1);
    assert_eq!(f64::from_bytes(&data, session_time_info).unwrap(), 0.5);
    group.bench_function("f64_session_time", |b| {
        b.iter(|| {
            let value = black_box(f64::from_bytes(&data, session_time_info).unwrap());
            black_box(value)
        })
    });

    let speed_info = support::require_variable(&schema, "Speed", VariableType::Float, 1);
    assert_eq!(f32::from_bytes(&data, speed_info).unwrap(), 0.5);
    group.bench_function("f32_speed", |b| {
        b.iter(|| {
            let value = black_box(f32::from_bytes(&data, speed_info).unwrap());
            black_box(value)
        })
    });

    let gear_info = support::require_variable(&schema, "Gear", VariableType::Integer, 1);
    assert_eq!(i32::from_bytes(&data, gear_info).unwrap(), 1);
    group.bench_function("i32_gear", |b| {
        b.iter(|| {
            let value = black_box(i32::from_bytes(&data, gear_info).unwrap());
            black_box(value)
        })
    });

    let session_tick_info =
        support::require_variable(&schema, "SessionTick", VariableType::Integer, 1);
    assert_eq!(i32::from_bytes(&data, session_tick_info).unwrap(), 1);
    group.bench_function("i32_session_tick", |b| {
        b.iter(|| {
            let value = black_box(i32::from_bytes(&data, session_tick_info).unwrap());
            black_box(value)
        })
    });

    let driver_marker_info =
        support::require_variable(&schema, "DriverMarker", VariableType::Boolean, 1);
    assert!(bool::from_bytes(&data, driver_marker_info).unwrap());
    group.bench_function("bool_driver_marker", |b| {
        b.iter(|| {
            let value = black_box(bool::from_bytes(&data, driver_marker_info).unwrap());
            black_box(value)
        })
    });

    group.finish();
}

/// Measure fresh typed-vector decoding for three 72-element arrays.
fn bench_array_extraction(c: &mut Criterion) {
    let (data, schema) = load_test_data();

    let mut group = c.benchmark_group("array_extraction");
    group.throughput(Throughput::Elements(72));

    // Benchmark the 72-element CarIdx arrays in the captured live schema.
    let lap_dist_pct_info =
        support::require_variable(&schema, "CarIdxLapDistPct", VariableType::Float, 72);
    let lap_distances = Vec::<f32>::from_bytes(&data, lap_dist_pct_info).unwrap();
    assert_eq!(lap_distances.len(), 72);
    assert_eq!(lap_distances[0], 0.5);
    assert_eq!(lap_distances[71], 71.5);
    group.bench_function(BenchmarkId::new("f32_array", 72), |b| {
        b.iter(|| {
            let value: Vec<f32> =
                black_box(Vec::<f32>::from_bytes(&data, lap_dist_pct_info).unwrap());
            black_box(value)
        })
    });

    let track_surface_info =
        support::require_variable(&schema, "CarIdxTrackSurface", VariableType::Integer, 72);
    let track_surfaces = Vec::<i32>::from_bytes(&data, track_surface_info).unwrap();
    assert_eq!(track_surfaces.len(), 72);
    assert_eq!(track_surfaces[0], 1);
    assert_eq!(track_surfaces[71], 72);
    group.bench_function(BenchmarkId::new("i32_array", 72), |b| {
        b.iter(|| {
            let value: Vec<i32> =
                black_box(Vec::<i32>::from_bytes(&data, track_surface_info).unwrap());
            black_box(value)
        })
    });

    let on_pit_road_info =
        support::require_variable(&schema, "CarIdxOnPitRoad", VariableType::Boolean, 72);
    let pit_road = Vec::<bool>::from_bytes(&data, on_pit_road_info).unwrap();
    assert_eq!(pit_road.len(), 72);
    assert!(pit_road[0]);
    assert!(!pit_road[1]);
    group.bench_function(BenchmarkId::new("bool_array", 72), |b| {
        b.iter(|| {
            let value: Vec<bool> =
                black_box(Vec::<bool>::from_bytes(&data, on_pit_road_info).unwrap());
            black_box(value)
        })
    });

    group.finish();
}

/// Separate bitfield decoding cost from operations on an existing value.
fn bench_bitfield_operations(c: &mut Criterion) {
    let (data, schema) = load_test_data();

    let mut group = c.benchmark_group("bitfield_operations");

    let bitfield_info =
        support::require_variable(&schema, "SessionFlags", VariableType::BitField, 1);
    let bitfield = BitField::from_bytes(&data, bitfield_info).unwrap();
    assert_eq!(bitfield.value(), 1);

    group.bench_function("bitfield_extraction", |b| {
        b.iter(|| {
            let bf = black_box(BitField::from_bytes(&data, bitfield_info).unwrap());
            black_box(bf)
        })
    });

    group.bench_function("bitfield_is_set", |b| {
        b.iter(|| {
            let is_set = black_box(bitfield.is_set(0));
            black_box(is_set)
        })
    });

    group.bench_function("bitfield_has_flag", |b| {
        b.iter(|| {
            let has_flag = black_box(bitfield.has_flag(0x00000001));
            black_box(has_flag)
        })
    });

    group.bench_function("bitfield_value", |b| {
        b.iter(|| {
            let value = black_box(bitfield.value());
            black_box(value)
        })
    });

    group.finish();
}

/// Compare successful extraction with deliberate bounds-error paths.
fn bench_bounds_checking(c: &mut Criterion) {
    let (data, schema) = load_test_data();

    let mut group = c.benchmark_group("bounds_checking");

    let speed_info = support::require_variable(&schema, "Speed", VariableType::Float, 1);
    let mut invalid_scalar_info = speed_info.clone();
    invalid_scalar_info.offset = data.len();
    assert!(f32::from_bytes(&data, &invalid_scalar_info).is_err());

    group.bench_function("valid_scalar", |b| {
        b.iter(|| {
            let result = black_box(f32::from_bytes(&data, speed_info));
            black_box(result)
        })
    });

    group.bench_function("invalid_scalar", |b| {
        b.iter(|| {
            let result = black_box(f32::from_bytes(&data, &invalid_scalar_info));
            black_box(result)
        })
    });

    let lap_dist_pct_info =
        support::require_variable(&schema, "CarIdxLapDistPct", VariableType::Float, 72);
    let mut invalid_array_info = lap_dist_pct_info.clone();
    invalid_array_info.offset = data.len();
    assert!(Vec::<f32>::from_bytes(&data, &invalid_array_info).is_err());

    group.bench_function("invalid_array_72", |b| {
        b.iter(|| {
            let result = black_box(Vec::<f32>::from_bytes(&data, &invalid_array_info));
            black_box(result)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_scalar_extraction,
    bench_array_extraction,
    bench_bitfield_operations,
    bench_bounds_checking
);
criterion_main!(benches);
