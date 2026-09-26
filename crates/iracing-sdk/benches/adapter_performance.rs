//! Performance benchmarks for dynamic and derived frame adapters.
//!
//! # What is being measured
//!
//! Every group uses the deterministic frame built from the checked-in live
//! variable-schema capture. Frame generation and adapter schema validation
//! happen before Criterion starts timing. The timed operations begin with an
//! already available [`FramePacket`] and exercise the public adapter or lookup
//! APIs that an application would call for each frame.
//!
//! The groups answer different questions:
//!
//! - `dynamic_frame/adapt` measures creation of a dynamic view by cloning the
//!   packet's shared data and schema handles; it does not decode every value.
//! - The remaining `dynamic_frame` cases measure representative scalar and
//!   array lookups on an existing view. The array case creates a fresh
//!   `Vec<f32>` per iteration.
//! - `derived_adapters` compares fresh typed output construction for adapters
//!   containing 5, 20, and 47 fields. Their validation plans are prepared once.
//!
//! Assertions before the timed loops verify that captured variables exist and
//! produce the expected deterministic values. Timed outputs are passed to
//! [`std::hint::black_box`] so the compiler cannot discard adapter work.
//!
//! # Reading results
//!
//! These are in-memory adaptation and lookup measurements, not end-to-end live
//! telemetry latency. They exclude frame acquisition, connection and provider
//! work, scheduling, subscription delivery, session parsing, serialization,
//! and application processing. Results depend on the captured schema, build,
//! machine, and allocator.
//!
//! Run this target with:
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench adapter-performance
//! ```

#![allow(dead_code)] // JUSTIFICATION: Benchmark frame structs are exercised through generated adapters; fields stay unread by the harness.

mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iracing_sdk::{
    DynamicFrame, IRacingTelemetryFrame, VariableSchema,
    adapters::{AdapterValidation, FrameAdapter},
    types::FramePacket,
};
use std::hint::black_box;
use std::sync::Arc;
use support::workloads::ConsumerFrame47;

// Small adapter (5 fields) - minimal overhead baseline
#[derive(IRacingTelemetryFrame, Debug, Clone)]
struct SmallFrame {
    #[field_name = "Speed"]
    speed: f32,

    #[field_name = "Gear"]
    gear: i32,

    #[field_name = "RPM"]
    rpm: f32,

    #[field_name = "Throttle"]
    throttle: f32,

    #[field_name = "Brake"]
    brake: f32,
}

// Medium adapter (20 fields) - typical dashboard use case
#[derive(IRacingTelemetryFrame, Debug, Clone)]
struct MediumFrame {
    // Core telemetry
    #[field_name = "Speed"]
    speed: f32,
    #[field_name = "Gear"]
    gear: i32,
    #[field_name = "RPM"]
    rpm: f32,
    #[field_name = "Throttle"]
    throttle: f32,
    #[field_name = "Brake"]
    brake: f32,
    #[field_name = "Clutch"]
    clutch: f32,
    #[field_name = "SteeringWheelAngle"]
    steering: f32,

    // Lap data
    #[field_name = "Lap"]
    lap: i32,
    #[field_name = "LapDist"]
    lap_dist: f32,
    #[field_name = "LapDistPct"]
    lap_dist_pct: f32,
    #[field_name = "LapCurrentLapTime"]
    current_lap_time: f32,
    #[field_name = "LapLastLapTime"]
    last_lap_time: f32,
    #[field_name = "LapBestLapTime"]
    best_lap_time: f32,

    // Session
    #[field_name = "SessionTime"]
    session_time: f64,
    #[field_name = "SessionTick"]
    session_tick: i32,
    #[field_name = "SessionNum"]
    session_num: i32,
    #[field_name = "SessionState"]
    session_state: i32,

    // Position
    #[field_name = "VelocityX"]
    velocity_x: f32,
    #[field_name = "VelocityY"]
    velocity_y: f32,
    #[field_name = "VelocityZ"]
    velocity_z: f32,
}

/// Get a deterministic packet with the captured full live-frame layout.
fn get_test_frame() -> (FramePacket, Arc<VariableSchema>) {
    let fixture = support::full_frame_fixture();
    let packet = fixture.packet();
    (packet, fixture.schema)
}

/// Build and verify a fully mapped validation plan outside timed loops.
fn require_complete_validation<T: FrameAdapter>(
    schema: &VariableSchema,
    expected_fields: usize,
) -> AdapterValidation {
    let validation = T::validate_schema(schema)
        .unwrap_or_else(|error| panic!("full-frame adapter validation failed: {error}"));

    assert_eq!(validation.field_count(), expected_fields);
    assert!(
        validation
            .extraction_plan
            .iter()
            .all(|extraction| extraction.var_info().is_some()),
        "full-frame adapter benchmark would exercise a missing/default field"
    );

    validation
}

/// Measure dynamic-view construction and by-name access on an existing view.
fn bench_dynamic_frame(c: &mut Criterion) {
    let (packet, schema) = get_test_frame();

    // Pre-validate for DynamicFrame
    let validation =
        DynamicFrame::validate_schema(&schema).expect("DynamicFrame validation failed");

    let mut group = c.benchmark_group("dynamic_frame");

    group.bench_function("adapt", |b| {
        b.iter(|| {
            let frame = DynamicFrame::adapt(black_box(&packet), black_box(&validation));
            black_box(frame)
        })
    });

    // Create frame once for field access benchmarks
    let frame = DynamicFrame::adapt(&packet, &validation);

    assert_eq!(frame.f32("Speed"), Some(0.5));
    let lap_dist: Option<Vec<f32>> = frame.get("CarIdxLapDistPct");
    assert_eq!(lap_dist.as_ref().map(Vec::len), Some(72));
    group.bench_function("scalar_hit", |b| {
        b.iter(|| {
            let speed = black_box(frame.f32("Speed"));
            black_box(speed)
        })
    });

    group.bench_function("array_hit_72", |b| {
        b.iter(|| {
            let lap_dist: Option<Vec<f32>> = black_box(frame.get("CarIdxLapDistPct"));
            black_box(lap_dist)
        })
    });

    group.finish();
}

/// Measure fresh typed outputs for increasing derived-adapter field counts.
fn bench_derived_adapters(c: &mut Criterion) {
    let (packet, schema) = get_test_frame();

    let mut group = c.benchmark_group("derived_adapters");

    let small_validation = require_complete_validation::<SmallFrame>(&schema, 5);
    group.bench_function(BenchmarkId::new("small_frame", "5_fields"), |b| {
        b.iter(|| {
            let frame = SmallFrame::adapt(black_box(&packet), black_box(&small_validation));
            black_box(frame)
        })
    });

    let medium_validation = require_complete_validation::<MediumFrame>(&schema, 20);
    group.bench_function(BenchmarkId::new("medium_frame", "20_fields"), |b| {
        b.iter(|| {
            let frame = MediumFrame::adapt(black_box(&packet), black_box(&medium_validation));
            black_box(frame)
        })
    });

    let large_validation = require_complete_validation::<ConsumerFrame47>(&schema, 47);
    group.bench_function(BenchmarkId::new("large_frame", "47_fields"), |b| {
        b.iter(|| {
            let frame = ConsumerFrame47::adapt(black_box(&packet), black_box(&large_validation));
            black_box(frame)
        })
    });

    group.finish();
}

criterion_group!(benches, bench_dynamic_frame, bench_derived_adapters);
criterion_main!(benches);
