//! Shared benchmark workload definitions that must remain comparable across
//! multiple Criterion targets.
//!
//! Keeping the 47-field adapter here prevents the adapter microbenchmarks and
//! aggregate consumer benchmark from silently evolving into different shapes.

use iracing_sdk::{
    IRacingTelemetryFrame, VariableInfo, VariableSchema, VariableType,
    adapters::{AdapterValidation, FrameAdapter},
    types::{FramePacket, VarData},
};

pub const CONSUMER_FIELD_COUNT: usize = 47;
pub const CAR_INDEX_COUNT: usize = 72;
pub const CONSUMER_ELEMENT_COUNT: u64 = CONSUMER_FIELD_COUNT as u64 + 3 * CAR_INDEX_COUNT as u64;

/// Representative scalar telemetry consumer shared by the adapter and
/// aggregate frame-parsing benchmarks.
#[derive(IRacingTelemetryFrame, Debug, Clone)]
pub struct ConsumerFrame47 {
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
    #[field_name = "SessionTimeRemain"]
    time_remain: Option<f64>,
    #[field_name = "ReplayFrameNum"]
    replay_frame: Option<i32>,
    #[field_name = "IsReplayPlaying"]
    is_replay: Option<bool>,
}

/// The representative adapter plus the source tick used for latency correlation.
pub struct TimedConsumerFrame47 {
    pub tick: u32,
    frame: ConsumerFrame47,
}

impl FrameAdapter for TimedConsumerFrame47 {
    fn validate_schema(schema: &VariableSchema) -> iracing_sdk::Result<AdapterValidation> {
        ConsumerFrame47::validate_schema(schema)
    }

    fn adapt(packet: &FramePacket, validation: &AdapterValidation) -> Self {
        Self {
            tick: packet.tick,
            frame: ConsumerFrame47::adapt(packet, validation),
        }
    }
}

impl TimedConsumerFrame47 {
    /// Return an opaque reference that keeps the adapted 47-field payload live.
    pub fn frame_marker(&self) -> &ConsumerFrame47 {
        &self.frame
    }
}

pub struct ConsumerWorkload {
    /// Precomputed mapping for all 47 typed adapter fields.
    pub validation: AdapterValidation,
    /// Metadata for the 72-element lap-distance array.
    pub lap_dist_pct: VariableInfo,
    /// Metadata for the 72-element track-surface array.
    pub track_surface: VariableInfo,
    /// Metadata for the 72-element pit-road-state array.
    pub on_pit_road: VariableInfo,
}

/// Prepare and exhaustively verify the representative consumer before timing.
pub fn prepare_consumer_workload(
    packet: &FramePacket,
    schema: &VariableSchema,
) -> ConsumerWorkload {
    let validation = ConsumerFrame47::validate_schema(schema)
        .unwrap_or_else(|error| panic!("consumer benchmark validation failed: {error}"));
    assert_eq!(validation.field_count(), CONSUMER_FIELD_COUNT);
    assert!(
        validation
            .extraction_plan
            .iter()
            .all(|extraction| extraction.var_info().is_some()),
        "consumer benchmark would exercise a missing or defaulted adapter field"
    );

    let lap_dist_pct = super::require_variable(
        schema,
        "CarIdxLapDistPct",
        VariableType::Float32,
        CAR_INDEX_COUNT,
    )
    .clone();
    let track_surface = super::require_variable(
        schema,
        "CarIdxTrackSurface",
        VariableType::Int32,
        CAR_INDEX_COUNT,
    )
    .clone();
    let on_pit_road = super::require_variable(
        schema,
        "CarIdxOnPitRoad",
        VariableType::Bool,
        CAR_INDEX_COUNT,
    )
    .clone();

    let workload = ConsumerWorkload {
        validation,
        lap_dist_pct,
        track_surface,
        on_pit_road,
    };
    verify_consumer(packet, &workload);
    workload
}

fn verify_consumer(packet: &FramePacket, workload: &ConsumerWorkload) {
    let frame = ConsumerFrame47::adapt(packet, &workload.validation);

    assert_eq!(frame.speed, 0.5);
    assert_eq!(frame.gear, 1);
    assert_eq!(frame.rpm, 0.5);
    assert_eq!(frame.throttle, 0.5);
    assert_eq!(frame.brake, 0.5);
    assert_eq!(frame.clutch, 0.5);
    assert_eq!(frame.steering, 0.5);
    assert_eq!(frame.lap, 1);
    assert_eq!(frame.lap_dist, 0.5);
    assert_eq!(frame.lap_dist_pct, 0.5);
    assert_eq!(frame.current_lap_time, 0.5);
    assert_eq!(frame.last_lap_time, 0.5);
    assert_eq!(frame.best_lap_time, 0.5);
    assert_eq!(frame.session_time, 0.5);
    assert_eq!(frame.session_tick, 1);
    assert_eq!(frame.session_num, 1);
    assert_eq!(frame.session_state, 1);
    assert_eq!(frame.velocity_x, 0.5);
    assert_eq!(frame.velocity_y, 0.5);
    assert_eq!(frame.velocity_z, 0.5);
    assert_eq!(frame.yaw_rate, 0.5);
    assert_eq!(frame.pitch, 0.5);
    assert_eq!(frame.roll, 0.5);
    assert_eq!(frame.pitch_rate, 0.5);
    assert_eq!(frame.roll_rate, 0.5);
    assert_eq!(frame.steering_torque, 0.5);
    assert_eq!(frame.fuel, Some(0.5));
    assert_eq!(frame.fuel_pct, Some(0.5));
    assert_eq!(frame.fuel_use, Some(0.5));
    assert_eq!(frame.water_temp, Some(0.5));
    assert_eq!(frame.oil_temp, Some(0.5));
    assert_eq!(frame.oil_press, Some(0.5));
    assert_eq!(frame.lf_temp_cl, Some(0.5));
    assert_eq!(frame.lf_temp_cm, Some(0.5));
    assert_eq!(frame.lf_temp_cr, Some(0.5));
    assert_eq!(frame.rf_temp_cl, Some(0.5));
    assert_eq!(frame.rf_temp_cm, Some(0.5));
    assert_eq!(frame.rf_temp_cr, Some(0.5));
    assert_eq!(frame.lr_temp_cl, Some(0.5));
    assert_eq!(frame.lr_temp_cm, Some(0.5));
    assert_eq!(frame.lr_temp_cr, Some(0.5));
    assert_eq!(frame.rr_temp_cl, Some(0.5));
    assert_eq!(frame.rr_temp_cm, Some(0.5));
    assert_eq!(frame.rr_temp_cr, Some(0.5));
    assert_eq!(frame.time_remain, Some(0.5));
    assert_eq!(frame.replay_frame, Some(1));
    assert_eq!(frame.is_replay, Some(true));

    let lap_dist_pct = Vec::<f32>::from_bytes(packet.data.as_ref(), &workload.lap_dist_pct)
        .expect("failed to verify CarIdxLapDistPct");
    assert_eq!(
        lap_dist_pct,
        (0..CAR_INDEX_COUNT)
            .map(|i| i as f32 + 0.5)
            .collect::<Vec<_>>()
    );

    let track_surface = Vec::<i32>::from_bytes(packet.data.as_ref(), &workload.track_surface)
        .expect("failed to verify CarIdxTrackSurface");
    assert_eq!(
        track_surface,
        (1..=CAR_INDEX_COUNT as i32).collect::<Vec<_>>()
    );

    let on_pit_road = Vec::<bool>::from_bytes(packet.data.as_ref(), &workload.on_pit_road)
        .expect("failed to verify CarIdxOnPitRoad");
    assert_eq!(
        on_pit_road,
        (0..CAR_INDEX_COUNT)
            .map(|index| index.is_multiple_of(2))
            .collect::<Vec<_>>()
    );
}
