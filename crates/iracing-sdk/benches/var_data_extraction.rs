//! Benchmarks for low-level VarData extraction from real telemetry
//!
//! Tests parsing performance for:
//! - Scalar types (f32, i32, u32, bool) from real IBT data
//! - Array types (Vec<f32>, Vec<i32>) from CarIdx arrays
//! - BitField operations on session flags
//! - Bounds checking overhead
//!
//! Platform: Cross-platform (uses real IBT test files, CI-safe)

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use pitwall::IbtReader;
use pitwall::test_utils::get_smallest_ibt_test_file;
use pitwall::types::{BitField, VarData};
use std::hint::black_box;

/// Load real frame data and variable info for benchmarking
fn load_test_data() -> (Vec<u8>, pitwall::VariableSchema) {
    let ibt_file = get_smallest_ibt_test_file().expect("No IBT test files found");
    let mut reader = IbtReader::open(&ibt_file).expect("Failed to open IBT file");

    let schema = reader.variables().clone();

    let (data, _tick, _session_version) =
        reader.read_next_frame().expect("Failed to read frame").expect("No frames in IBT");

    (data, schema)
}

fn bench_scalar_extraction(c: &mut Criterion) {
    let (data, schema) = load_test_data();

    let mut group = c.benchmark_group("scalar_extraction");

    // Benchmark common scalar types with real variables
    if let Some(speed_info) = schema.get_variable("Speed") {
        group.bench_function("f32_speed", |b| {
            b.iter(|| {
                let value = black_box(f32::from_bytes(&data, speed_info).unwrap());
                black_box(value)
            })
        });
    }

    if let Some(gear_info) = schema.get_variable("Gear") {
        group.bench_function("i32_gear", |b| {
            b.iter(|| {
                let value = black_box(i32::from_bytes(&data, gear_info).unwrap());
                black_box(value)
            })
        });
    }

    if let Some(session_tick_info) = schema.get_variable("SessionTick") {
        group.bench_function("i32_session_tick", |b| {
            b.iter(|| {
                let value = black_box(i32::from_bytes(&data, session_tick_info).unwrap());
                black_box(value)
            })
        });
    }

    if let Some(driver_marker_info) = schema.get_variable("DriverMarker") {
        group.bench_function("bool_driver_marker", |b| {
            b.iter(|| {
                let value = black_box(bool::from_bytes(&data, driver_marker_info).unwrap());
                black_box(value)
            })
        });
    }

    group.finish();
}

fn bench_array_extraction(c: &mut Criterion) {
    let (data, schema) = load_test_data();

    let mut group = c.benchmark_group("array_extraction");

    // Benchmark array variables (typically 64 elements for CarIdx arrays)
    if let Some(lap_dist_pct_info) = schema.get_variable("CarIdxLapDistPct") {
        let element_count = lap_dist_pct_info.count;
        group.bench_function(BenchmarkId::new("f32_array", element_count), |b| {
            b.iter(|| {
                let value: Vec<f32> =
                    black_box(Vec::<f32>::from_bytes(&data, lap_dist_pct_info).unwrap());
                black_box(value)
            })
        });
    }

    if let Some(track_surface_info) = schema.get_variable("CarIdxTrackSurface") {
        let element_count = track_surface_info.count;
        group.bench_function(BenchmarkId::new("i32_array", element_count), |b| {
            b.iter(|| {
                let value: Vec<i32> =
                    black_box(Vec::<i32>::from_bytes(&data, track_surface_info).unwrap());
                black_box(value)
            })
        });
    }

    if let Some(on_pit_road_info) = schema.get_variable("CarIdxOnPitRoad") {
        let element_count = on_pit_road_info.count;
        group.bench_function(BenchmarkId::new("bool_array", element_count), |b| {
            b.iter(|| {
                let value: Vec<bool> =
                    black_box(Vec::<bool>::from_bytes(&data, on_pit_road_info).unwrap());
                black_box(value)
            })
        });
    }

    group.finish();
}

fn bench_bitfield_operations(c: &mut Criterion) {
    let (data, schema) = load_test_data();

    let mut group = c.benchmark_group("bitfield_operations");

    // Find a real bitfield variable in the schema
    let bitfield_var =
        schema.variables.values().find(|v| matches!(v.data_type, pitwall::VariableType::BitField));

    if let Some(bitfield_info) = bitfield_var {
        if let Ok(bitfield) = BitField::from_bytes(&data, bitfield_info) {
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
        }
    }

    group.finish();
}

fn bench_bounds_checking(c: &mut Criterion) {
    let (data, schema) = load_test_data();

    let mut group = c.benchmark_group("bounds_checking");

    // Test bounds checking overhead on array access
    if let Some(lap_dist_pct_info) = schema.get_variable("CarIdxLapDistPct") {
        // Valid access (within bounds)
        group.bench_function("valid_array_access", |b| {
            b.iter(|| {
                let value: Vec<f32> =
                    black_box(Vec::<f32>::from_bytes(&data, lap_dist_pct_info).unwrap());
                black_box(value)
            })
        });
    }

    // Test scalar bounds checking
    if let Some(speed_info) = schema.get_variable("Speed") {
        group.bench_function("scalar_bounds_check", |b| {
            b.iter(|| {
                let result = black_box(f32::from_bytes(&data, speed_info));
                black_box(result)
            })
        });
    }

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
