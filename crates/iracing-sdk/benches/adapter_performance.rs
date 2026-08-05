//! Benchmarks for frame adapter performance using a captured live-frame schema
//!
//! Tests the <100μs frame construction latency goal for:
//! - DynamicFrame adapter (HashMap-based field lookups)
//! - Derived adapters with varying field counts (5, 20, 47 fields)
//! - Optional vs required field extraction overhead
//! - Array field extraction performance
//!
//! Platform: Cross-platform (uses the checked-in live variable schema, CI-safe)

#![allow(dead_code)] // JUSTIFICATION: Benchmark frame structs are exercised through generated adapters; fields stay unread by the harness.

mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iracing_sdk::{
    DynamicFrame, IRacingTelemetryFrame, VariableSchema,
    adapters::{AdapterValidation, FrameAdapter},
    types::FramePacket,
};
use std::collections::HashMap;
use std::hint::black_box;
use std::sync::Arc;

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

// Large adapter (47 fields) - comprehensive telemetry logging
#[derive(IRacingTelemetryFrame, Debug, Clone)]
struct LargeFrame {
    // All fields from MediumFrame
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
    #[field_name = "SessionTime"]
    session_time: f64,
    #[field_name = "SessionTick"]
    session_tick: i32,
    #[field_name = "SessionNum"]
    session_num: i32,
    #[field_name = "SessionState"]
    session_state: i32,
    #[field_name = "VelocityX"]
    velocity_x: f32,
    #[field_name = "VelocityY"]
    velocity_y: f32,
    #[field_name = "VelocityZ"]
    velocity_z: f32,

    // Additional fields for large frame
    #[field_name = "YawRate"]
    yaw_rate: f32,
    #[field_name = "Pitch"]
    pitch: f32,
    #[field_name = "Roll"]
    roll: f32,
    #[field_name = "PitchRate"]
    pitch_rate: f32,
    #[field_name = "RollRate"]
    roll_rate: f32,
    #[field_name = "SteeringWheelTorque"]
    steering_torque: f32,

    // Engine/fuel
    #[field_name = "FuelLevel"]
    fuel: Option<f32>,
    #[field_name = "FuelLevelPct"]
    fuel_pct: Option<f32>,
    #[field_name = "FuelUsePerHour"]
    fuel_use: Option<f32>,
    #[field_name = "WaterTemp"]
    water_temp: Option<f32>,
    #[field_name = "OilTemp"]
    oil_temp: Option<f32>,
    #[field_name = "OilPress"]
    oil_press: Option<f32>,

    // Tires
    #[field_name = "LFtempCL"]
    lf_temp_cl: Option<f32>,
    #[field_name = "LFtempCM"]
    lf_temp_cm: Option<f32>,
    #[field_name = "LFtempCR"]
    lf_temp_cr: Option<f32>,
    #[field_name = "RFtempCL"]
    rf_temp_cl: Option<f32>,
    #[field_name = "RFtempCM"]
    rf_temp_cm: Option<f32>,
    #[field_name = "RFtempCR"]
    rf_temp_cr: Option<f32>,
    #[field_name = "LRtempCL"]
    lr_temp_cl: Option<f32>,
    #[field_name = "LRtempCM"]
    lr_temp_cm: Option<f32>,
    #[field_name = "LRtempCR"]
    lr_temp_cr: Option<f32>,
    #[field_name = "RRtempCL"]
    rr_temp_cl: Option<f32>,
    #[field_name = "RRtempCM"]
    rr_temp_cm: Option<f32>,
    #[field_name = "RRtempCR"]
    rr_temp_cr: Option<f32>,

    // Timing
    #[field_name = "SessionTimeRemain"]
    time_remain: Option<f64>,
    #[field_name = "ReplayFrameNum"]
    replay_frame: Option<i32>,
    #[field_name = "IsReplayPlaying"]
    is_replay: Option<bool>,
}

// Adapter testing optional fields overhead
#[derive(IRacingTelemetryFrame, Debug, Clone)]
struct OptionalFieldsFrame {
    #[field_name = "Speed"]
    speed: f32,

    #[field_name = "Gear"]
    gear: Option<i32>,

    #[field_name = "FuelLevel"]
    fuel: Option<f32>,

    #[field_name = "FuelLevelPct"]
    fuel_pct: Option<f32>,

    #[field_name = "WaterTemp"]
    water_temp: Option<f32>,
}

/// Get a deterministic packet with the captured full live-frame layout.
fn get_test_frame() -> (FramePacket, Arc<VariableSchema>) {
    let fixture = support::full_frame_fixture();
    let packet = fixture.packet();
    (packet, fixture.schema)
}

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
    assert_eq!(frame.f32("BenchmarkMissingScalar"), None);
    let missing_array: Option<Vec<f32>> = frame.get("BenchmarkMissingArray");
    assert_eq!(missing_array, None);

    group.bench_function("scalar_hit", |b| {
        b.iter(|| {
            let speed = black_box(frame.f32("Speed"));
            black_box(speed)
        })
    });

    group.bench_function("scalar_miss", |b| {
        b.iter(|| {
            let value = black_box(frame.f32("BenchmarkMissingScalar"));
            black_box(value)
        })
    });

    group.bench_function("array_hit_72", |b| {
        b.iter(|| {
            let lap_dist: Option<Vec<f32>> = black_box(frame.get("CarIdxLapDistPct"));
            black_box(lap_dist)
        })
    });

    group.bench_function("array_miss", |b| {
        b.iter(|| {
            let value: Option<Vec<f32>> = black_box(frame.get("BenchmarkMissingArray"));
            black_box(value)
        })
    });

    group.finish();
}

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

    let large_validation = require_complete_validation::<LargeFrame>(&schema, 47);
    group.bench_function(BenchmarkId::new("large_frame", "47_fields"), |b| {
        b.iter(|| {
            let frame = LargeFrame::adapt(black_box(&packet), black_box(&large_validation));
            black_box(frame)
        })
    });

    group.finish();
}

fn bench_optional_fields(c: &mut Criterion) {
    let (packet, schema) = get_test_frame();

    let mut group = c.benchmark_group("optional_fields");

    let present_validation = require_complete_validation::<OptionalFieldsFrame>(&schema, 5);
    let present_frame = OptionalFieldsFrame::adapt(&packet, &present_validation);
    assert!(present_frame.gear.is_some());
    assert!(present_frame.fuel.is_some());
    assert!(present_frame.fuel_pct.is_some());
    assert!(present_frame.water_temp.is_some());

    group.bench_function("all_present", |b| {
        b.iter(|| {
            let frame =
                OptionalFieldsFrame::adapt(black_box(&packet), black_box(&present_validation));
            black_box(frame)
        })
    });

    let mut missing_schema = schema.as_ref().clone();
    for name in ["Gear", "FuelLevel", "FuelLevelPct", "WaterTemp"] {
        missing_schema.variables.remove(name);
    }
    let missing_schema = Arc::new(missing_schema);
    let missing_packet = FramePacket::new(
        packet.data.to_vec(),
        packet.tick,
        packet.session_version,
        Arc::clone(&missing_schema),
    );
    let missing_validation = OptionalFieldsFrame::validate_schema(&missing_schema).unwrap();
    let missing_frame = OptionalFieldsFrame::adapt(&missing_packet, &missing_validation);
    assert!(missing_frame.gear.is_none());
    assert!(missing_frame.fuel.is_none());
    assert!(missing_frame.fuel_pct.is_none());
    assert!(missing_frame.water_temp.is_none());

    group.bench_function("all_missing", |b| {
        b.iter(|| {
            let frame = OptionalFieldsFrame::adapt(
                black_box(&missing_packet),
                black_box(&missing_validation),
            );
            black_box(frame)
        })
    });

    group.finish();
}

fn bench_type_defaults(c: &mut Criterion) {
    let (packet, _) = get_test_frame();
    let schema = Arc::new(VariableSchema::new(HashMap::new(), packet.data.len()).unwrap());
    let packet = FramePacket::new(
        packet.data.to_vec(),
        packet.tick,
        packet.session_version,
        Arc::clone(&schema),
    );
    let validation = SmallFrame::validate_schema(&schema).unwrap();
    let frame = SmallFrame::adapt(&packet, &validation);
    assert_eq!(frame.speed, 0.0);
    assert_eq!(frame.gear, 0);

    c.bench_function("missing_fields/type_defaults_5_fields", |b| {
        b.iter(|| {
            let frame = SmallFrame::adapt(black_box(&packet), black_box(&validation));
            black_box(frame)
        })
    });
}

criterion_group!(
    benches,
    bench_dynamic_frame,
    bench_derived_adapters,
    bench_optional_fields,
    bench_type_defaults
);
criterion_main!(benches);
