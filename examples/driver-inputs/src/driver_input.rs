use iracing_sdk::types::irsdk::{IncidentFlags, SessionFlags, SessionState, TrackLocation};
use iracing_sdk_derive::IRacingTelemetryFrame;
use serde::Serialize;

#[derive(IRacingTelemetryFrame, Debug, Clone, Copy, Serialize)]
pub struct DriverInput {
    #[field_name = "Lap"]
    #[fail_if_missing]
    pub lap_number: i32,

    #[field_name = "SessionState"]
    pub session_state: SessionState,

    /// Input & Output.
    /// GPS vehicle speed. Unit: m/s.
    #[field_name = "Speed"]
    speed: f32,
    /// Engine rpm. Unit: revs/min.
    #[field_name = "RPM"]
    rpm: f32,
    /// -1=reverse  0=neutral  1..n=current gear.
    #[field_name = "Gear"]
    gear: i32,

    /// Pitch orientation. Unit: rad.
    #[field_name = "Pitch"]
    pitch: f32,
    /// Pitch rate. Unit: rad/s.
    #[field_name = "PitchRate"]
    pitch_rate: f32,

    /// Yaw orientation. Unit: rad.
    #[field_name = "Yaw"]
    yaw: f32,
    /// Yaw rate. Unit: rad/s.
    #[field_name = "YawRate"]
    yaw_rate: f32,
    /// Yaw orientation relative to north. Unit: rad.
    #[field_name = "YawNorth"]
    yaw_north: f32,

    /// Roll orientation. Unit: rad.
    #[field_name = "Roll"]
    roll: f32,
    /// Roll rate. Unit: rad/s.
    #[field_name = "RollRate"]
    roll_rate: f32,

    /// X velocity. Unit: m/s.
    #[field_name = "VelocityX"]
    velocity_x: f32,
    /// Y velocity. Unit: m/s.
    #[field_name = "VelocityY"]
    velocity_y: f32,
    /// Z velocity. Unit: m/s.
    #[field_name = "VelocityZ"]
    velocity_z: f32,

    /// Lateral acceleration (including gravity). Unit: m/s^2.
    #[field_name = "LatAccel"]
    lat_accel: f32,
    /// Longitudinal acceleration (including gravity). Unit: m/s^2.
    #[field_name = "LongAccel"]
    long_accel: f32,
    /// Vertical acceleration (including gravity). Unit: m/s^2.
    #[field_name = "VertAccel"]
    vert_accel: f32,

    /// Steering wheel angle. Unit: rad.
    #[field_name = "SteeringWheelAngle"]
    steering_wheel_angle: f32,
    /// Steering wheel max angle. Unit: rad.
    #[field_name = "SteeringWheelAngleMax"]
    steering_wheel_angle_max: f32,

    /// 0=off throttle to 1=full throttle. Unit: %.
    #[field_name = "Throttle"]
    throttle: f32,
    /// Raw throttle input 0=off throttle to 1=full throttle. Unit: %.
    #[field_name = "ThrottleRaw"]
    throttle_raw: f32,

    /// 0=brake released to 1=max pedal force. Unit: %.
    #[field_name = "Brake"]
    brake: f32,
    /// Raw brake input 0=brake released to 1=max pedal force. Unit: %.
    #[field_name = "BrakeRaw"]
    brake_raw: f32,
    /// true if abs is currently reducing brake force pressure.
    #[field_name = "BrakeABSActive"]
    brake_abs_active: bool,
    /// Percent of brake force reduction caused by ABS system. Unit: %.
    #[field_name = "BrakeABScutPct"]
    brake_abs_cut: f32,
    /// Raw handbrake input 0=handbrake released to 1=max force. Unit: %.
    #[field_name = "HandbrakeRaw"]
    handbrake_raw: f32,

    /// 0=disengaged to 1=fully engaged. Unit: %.
    #[field_name = "Clutch"]
    clutch: f32,
    #[field_name = "ClutchRaw"]
    clutch_raw: f32,

    /// !!!: iRacing uses EPSG:3857 for coordinates.
    /// Latitude in decimal degress. Unit: deg.
    #[field_name = "Lat"]
    latitude: f64,

    /// Longitude in decimal degress. Unit: deg.
    #[field_name = "Lon"]
    longitude: f64,

    /// Altitude. Unit: m.
    #[field_name = "Alt"]
    altitude: f32,

    #[field_name = "PlayerTrackSurface"]
    #[fail_if_missing]
    pub track_location: TrackLocation,

    #[field_name = "IsOnTrack"]
    is_on_track: bool,

    #[field_name = "PitsOpen"]
    is_pits_open: bool,

    #[field_name = "OnPitRoad"]
    is_on_pit_road: bool,

    #[field_name = "PlayerCarInPitStall"]
    is_in_pit_stall: bool,

    #[field_name = "PlayerIncidents"]
    #[fail_if_missing]
    // #[calculated = "iracing_sdk::types::IncidentFlags::from_bits_retain"]
    pub player_incidents: IncidentFlags,

    #[field_name = "SessionFlags"]
    #[fail_if_missing]
    pub flags: SessionFlags,

    #[bitfield_map(
        name = "SessionFlags",
        decoder = "iracing_sdk::types::session_start_control_shown"
    )]
    pub has_start_control: bool,

    #[bitfield_map(
        name = "SessionFlags",
        decoder = "iracing_sdk::types::session_under_yellow"
    )]
    pub is_yellow: bool,

    #[bitfield_map(
        name = "SessionFlags",
        decoder = "iracing_sdk::types::session_under_caution"
    )]
    pub is_caution: bool,

    #[bitfield(
        name = "SessionFlags",
        has = "iracing_sdk::types::irsdk::SessionFlags::DEBRIS.bits()"
    )]
    pub is_debris: bool,

    #[bitfield(
        name = "SessionFlags",
        has = "iracing_sdk::types::irsdk::SessionFlags::BLUE.bits()"
    )]
    pub is_faster_car_approaching: bool,

    #[bitfield_map(
        name = "EngineWarnings",
        decoder = "iracing_sdk::types::engine_repairs_needed"
    )]
    pub has_repairs: bool,

    #[bitfield(
        name = "SessionFlags",
        has = "iracing_sdk::types::irsdk::SessionFlags::SERVICE_ALLOWED.bits()"
    )]
    pub is_servicible: bool,

    ///
    /// Tire wear
    ///
    #[field_name = "LFwearL"]
    left_front_wear_outside: f32,
    #[field_name = "LFwearM"]
    left_front_wear_middle: f32,
    #[field_name = "LFwearR"]
    left_front_wear_inside: f32,

    #[field_name = "RFwearL"]
    right_front_wear_outside: f32,
    #[field_name = "RFwearM"]
    right_front_wear_middle: f32,
    #[field_name = "RFwearR"]
    right_front_wear_inside: f32,

    #[field_name = "LRwearL"]
    left_rear_wear_outside: f32,
    #[field_name = "LRwearM"]
    left_rear_wear_middle: f32,
    #[field_name = "LRwearR"]
    left_rear_wear_inside: f32,

    #[field_name = "RRwearL"]
    right_rear_wear_outside: f32,
    #[field_name = "RRwearM"]
    right_rear_wear_middle: f32,
    #[field_name = "RRwearR"]
    right_rear_wear_inside: f32,
}
