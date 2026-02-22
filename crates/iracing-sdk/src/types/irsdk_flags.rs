//! Low-level IRSDK constants mirrored from `irsdk_defines.h`.
//!
//! These are raw numeric values that map 1:1 with iRacing's C++ SDK symbols.
//! Prefer typed wrappers from `irsdk_enums` and `irsdk_bitflags` for application code.

/// `enum irsdk_StatusField`
pub mod status_field {
    pub const CONNECTED: i32 = 0x0000_0001; // irsdk_stConnected
}

/// `enum irsdk_TrkLoc`
pub mod trk_loc {
    pub const NOT_IN_WORLD: i32 = -1; // irsdk_NotInWorld
    pub const OFF_TRACK: i32 = 0; // irsdk_OffTrack
    pub const IN_PIT_STALL: i32 = 1; // irsdk_InPitStall
    pub const APPROACHING_PITS: i32 = 2; // irsdk_AproachingPits
    pub const ON_TRACK: i32 = 3; // irsdk_OnTrack
}

/// `enum irsdk_TrkSurf`
pub mod trk_surf {
    pub const SURFACE_NOT_IN_WORLD: i32 = -1; // irsdk_SurfaceNotInWorld
    pub const UNDEFINED_MATERIAL: i32 = 0; // irsdk_UndefinedMaterial
    pub const ASPHALT1_MATERIAL: i32 = 1;
    pub const ASPHALT2_MATERIAL: i32 = 2;
    pub const ASPHALT3_MATERIAL: i32 = 3;
    pub const ASPHALT4_MATERIAL: i32 = 4;
    pub const CONCRETE1_MATERIAL: i32 = 5;
    pub const CONCRETE2_MATERIAL: i32 = 6;
    pub const RACING_DIRT1_MATERIAL: i32 = 7;
    pub const RACING_DIRT2_MATERIAL: i32 = 8;
    pub const PAINT1_MATERIAL: i32 = 9;
    pub const PAINT2_MATERIAL: i32 = 10;
    pub const RUMBLE1_MATERIAL: i32 = 11;
    pub const RUMBLE2_MATERIAL: i32 = 12;
    pub const RUMBLE3_MATERIAL: i32 = 13;
    pub const RUMBLE4_MATERIAL: i32 = 14;
    pub const GRASS1_MATERIAL: i32 = 15;
    pub const GRASS2_MATERIAL: i32 = 16;
    pub const GRASS3_MATERIAL: i32 = 17;
    pub const GRASS4_MATERIAL: i32 = 18;
    pub const DIRT1_MATERIAL: i32 = 19;
    pub const DIRT2_MATERIAL: i32 = 20;
    pub const DIRT3_MATERIAL: i32 = 21;
    pub const DIRT4_MATERIAL: i32 = 22;
    pub const SAND_MATERIAL: i32 = 23;
    pub const GRAVEL1_MATERIAL: i32 = 24;
    pub const GRAVEL2_MATERIAL: i32 = 25;
    pub const GRASSCRETE_MATERIAL: i32 = 26;
    pub const ASTROTURF_MATERIAL: i32 = 27;
}

/// `enum irsdk_SessionState`
pub mod session_state {
    pub const INVALID: i32 = 0; // irsdk_StateInvalid
    pub const GET_IN_CAR: i32 = 1; // irsdk_StateGetInCar
    pub const WARMUP: i32 = 2; // irsdk_StateWarmup
    pub const PARADE_LAPS: i32 = 3; // irsdk_StateParadeLaps
    pub const RACING: i32 = 4; // irsdk_StateRacing
    pub const CHECKERED: i32 = 5; // irsdk_StateCheckered
    pub const COOL_DOWN: i32 = 6; // irsdk_StateCoolDown
}

/// `enum irsdk_CarLeftRight`
pub mod car_left_right {
    pub const OFF: i32 = 0; // irsdk_LROff
    pub const CLEAR: i32 = 1; // irsdk_LRClear
    pub const CAR_LEFT: i32 = 2; // irsdk_LRCarLeft
    pub const CAR_RIGHT: i32 = 3; // irsdk_LRCarRight
    pub const CAR_LEFT_RIGHT: i32 = 4; // irsdk_LRCarLeftRight
    pub const TWO_CARS_LEFT: i32 = 5; // irsdk_LR2CarsLeft
    pub const TWO_CARS_RIGHT: i32 = 6; // irsdk_LR2CarsRight
}

/// `enum irsdk_PitSvStatus`
pub mod pit_sv_status {
    pub const NONE: i32 = 0; // irsdk_PitSvNone
    pub const IN_PROGRESS: i32 = 1; // irsdk_PitSvInProgress
    pub const COMPLETE: i32 = 2; // irsdk_PitSvComplete
    pub const TOO_FAR_LEFT: i32 = 100; // irsdk_PitSvTooFarLeft
    pub const TOO_FAR_RIGHT: i32 = 101; // irsdk_PitSvTooFarRight
    pub const TOO_FAR_FORWARD: i32 = 102; // irsdk_PitSvTooFarForward
    pub const TOO_FAR_BACK: i32 = 103; // irsdk_PitSvTooFarBack
    pub const BAD_ANGLE: i32 = 104; // irsdk_PitSvBadAngle
    pub const CANT_FIX_THAT: i32 = 105; // irsdk_PitSvCantFixThat
}

/// `enum irsdk_PaceMode`
pub mod pace_mode {
    pub const SINGLE_FILE_START: i32 = 0; // irsdk_PaceModeSingleFileStart
    pub const DOUBLE_FILE_START: i32 = 1; // irsdk_PaceModeDoubleFileStart
    pub const SINGLE_FILE_RESTART: i32 = 2; // irsdk_PaceModeSingleFileRestart
    pub const DOUBLE_FILE_RESTART: i32 = 3; // irsdk_PaceModeDoubleFileRestart
    pub const NOT_PACING: i32 = 4; // irsdk_PaceModeNotPacing
}

/// `enum irsdk_TrackWetness`
pub mod track_wetness {
    pub const UNKNOWN: i32 = 0; // irsdk_TrackWetness_UNKNOWN
    pub const DRY: i32 = 1;
    pub const MOSTLY_DRY: i32 = 2;
    pub const VERY_LIGHTLY_WET: i32 = 3;
    pub const LIGHTLY_WET: i32 = 4;
    pub const MODERATELY_WET: i32 = 5;
    pub const VERY_WET: i32 = 6;
    pub const EXTREMELY_WET: i32 = 7;
}

/// `enum irsdk_IncidentFlags`
pub mod incident {
    pub const REP_MASK: u32 = 0x0000_00FF; // IRSDK_INCIDENT_REP_MASK
    pub const PEN_MASK: u32 = 0x0000_FF00; // IRSDK_INCIDENT_PEN_MASK

    pub const REP_NO_REPORT: u8 = 0x00;
    pub const REP_OUT_OF_CONTROL: u8 = 0x01;
    pub const REP_OFF_TRACK: u8 = 0x02;
    pub const REP_OFF_TRACK_ONGOING: u8 = 0x03;
    pub const REP_CONTACT_WITH_WORLD: u8 = 0x04;
    pub const REP_COLLISION_WITH_WORLD: u8 = 0x05;
    pub const REP_COLLISION_WITH_WORLD_ONGOING: u8 = 0x06;
    pub const REP_CONTACT_WITH_CAR: u8 = 0x07;
    pub const REP_COLLISION_WITH_CAR: u8 = 0x08;

    pub const PEN_NONE: u8 = 0x00;
    pub const PEN_0X: u8 = 0x01;
    pub const PEN_1X: u8 = 0x02;
    pub const PEN_2X: u8 = 0x03;
    pub const PEN_4X: u8 = 0x04;
}

/// `enum irsdk_EngineWarnings`
pub mod engine_warnings {
    pub const WATER_TEMP_WARNING: u32 = 0x0001;
    pub const FUEL_PRESSURE_WARNING: u32 = 0x0002;
    pub const OIL_PRESSURE_WARNING: u32 = 0x0004;
    pub const ENGINE_STALLED: u32 = 0x0008;
    pub const PIT_SPEED_LIMITER: u32 = 0x0010;
    pub const REV_LIMITER_ACTIVE: u32 = 0x0020;
    pub const OIL_TEMP_WARNING: u32 = 0x0040;
    pub const MAND_REP_NEEDED: u32 = 0x0080;
    pub const OPT_REP_NEEDED: u32 = 0x0100;
}

/// `enum irsdk_Flags`
pub mod flags {
    pub const CHECKERED: u32 = 0x0000_0001;
    pub const WHITE: u32 = 0x0000_0002;
    pub const GREEN: u32 = 0x0000_0004;
    pub const YELLOW: u32 = 0x0000_0008;
    pub const RED: u32 = 0x0000_0010;
    pub const BLUE: u32 = 0x0000_0020;
    pub const DEBRIS: u32 = 0x0000_0040;
    pub const CROSSED: u32 = 0x0000_0080;
    pub const YELLOW_WAVING: u32 = 0x0000_0100;
    pub const ONE_LAP_TO_GREEN: u32 = 0x0000_0200;
    pub const GREEN_HELD: u32 = 0x0000_0400;
    pub const TEN_TO_GO: u32 = 0x0000_0800;
    pub const FIVE_TO_GO: u32 = 0x0000_1000;
    pub const RANDOM_WAVING: u32 = 0x0000_2000;
    pub const CAUTION: u32 = 0x0000_4000;
    pub const CAUTION_WAVING: u32 = 0x0000_8000;
    pub const BLACK: u32 = 0x0001_0000;
    pub const DISQUALIFY: u32 = 0x0002_0000;
    pub const SERVICIBLE: u32 = 0x0004_0000;
    pub const FURLED: u32 = 0x0008_0000;
    pub const REPAIR: u32 = 0x0010_0000;
    pub const DQ_SCORING_INVALID: u32 = 0x0020_0000;
    pub const START_HIDDEN: u32 = 0x1000_0000;
    pub const START_READY: u32 = 0x2000_0000;
    pub const START_SET: u32 = 0x4000_0000;
    pub const START_GO: u32 = 0x8000_0000;
}

/// Backward-compatible alias for legacy naming.
pub mod session_flags {
    pub const DQ_SCORING_INVALID: u32 = super::flags::DQ_SCORING_INVALID;
}

/// `enum irsdk_CameraState`
pub mod camera_state {
    pub const IS_SESSION_SCREEN: u32 = 0x0001;
    pub const IS_SCENIC_ACTIVE: u32 = 0x0002;
    pub const CAM_TOOL_ACTIVE: u32 = 0x0004;
    pub const UI_HIDDEN: u32 = 0x0008;
    pub const USE_AUTO_SHOT_SELECTION: u32 = 0x0010;
    pub const USE_TEMPORARY_EDITS: u32 = 0x0020;
    pub const USE_KEY_ACCELERATION: u32 = 0x0040;
    pub const USE_KEY_10X_ACCELERATION: u32 = 0x0080;
    pub const USE_MOUSE_AIM_MODE: u32 = 0x0100;
}

/// `enum irsdk_PitSvFlags`
pub mod pit_sv_flags {
    pub const LF_TIRE_CHANGE: u32 = 0x0001;
    pub const RF_TIRE_CHANGE: u32 = 0x0002;
    pub const LR_TIRE_CHANGE: u32 = 0x0004;
    pub const RR_TIRE_CHANGE: u32 = 0x0008;
    pub const FUEL_FILL: u32 = 0x0010;
    pub const WINDSHIELD_TEAROFF: u32 = 0x0020;
    pub const FAST_REPAIR: u32 = 0x0040;
}

/// `enum irsdk_PaceFlags`
pub mod pace_flags {
    pub const END_OF_LINE: u32 = 0x0001;
    pub const FREE_PASS: u32 = 0x0002;
    pub const WAVED_AROUND: u32 = 0x0004;
}

/// `enum irsdk_BroadcastMsg`
pub mod broadcast_msg {
    pub const CAM_SWITCH_POS: i32 = 0;
    pub const CAM_SWITCH_NUM: i32 = 1;
    pub const CAM_SET_STATE: i32 = 2;
    pub const REPLAY_SET_PLAY_SPEED: i32 = 3;
    pub const REPLAY_SET_PLAY_POSITION: i32 = 4;
    pub const REPLAY_SEARCH: i32 = 5;
    pub const REPLAY_SET_STATE: i32 = 6;
    pub const RELOAD_TEXTURES: i32 = 7;
    pub const CHAT_COMMAND: i32 = 8;
    pub const PIT_COMMAND: i32 = 9;
    pub const TELEM_COMMAND: i32 = 10;
    pub const FFB_COMMAND: i32 = 11;
    pub const REPLAY_SEARCH_SESSION_TIME: i32 = 12;
    pub const VIDEO_CAPTURE: i32 = 13;
    pub const LAST: i32 = 14;
}

/// `enum irsdk_ChatCommandMode`
pub mod chat_command_mode {
    pub const MACRO: i32 = 0;
    pub const BEGIN_CHAT: i32 = 1;
    pub const REPLY: i32 = 2;
    pub const CANCEL: i32 = 3;
}

/// `enum irsdk_PitCommandMode`
pub mod pit_command_mode {
    pub const CLEAR: i32 = 0;
    pub const WS: i32 = 1;
    pub const FUEL: i32 = 2;
    pub const LF: i32 = 3;
    pub const RF: i32 = 4;
    pub const LR: i32 = 5;
    pub const RR: i32 = 6;
    pub const CLEAR_TIRES: i32 = 7;
    pub const FR: i32 = 8;
    pub const CLEAR_WS: i32 = 9;
    pub const CLEAR_FR: i32 = 10;
    pub const CLEAR_FUEL: i32 = 11;
    pub const TC: i32 = 12;
}

/// `enum irsdk_TelemetryCommandMode`
pub mod telem_command_mode {
    pub const STOP: i32 = 0;
    pub const START: i32 = 1;
    pub const RESTART: i32 = 2;
}

/// `enum irsdk_RpyStateMode`
pub mod rpy_state_mode {
    pub const ERASE_TAPE: i32 = 0;
    pub const LAST: i32 = 1;
}

/// `enum irsdk_ReloadTexturesMode`
pub mod reload_textures_mode {
    pub const ALL: i32 = 0;
    pub const CAR_IDX: i32 = 1;
}

/// `enum irsdk_RpySrchMode`
pub mod rpy_srch_mode {
    pub const TO_START: i32 = 0;
    pub const TO_END: i32 = 1;
    pub const PREV_SESSION: i32 = 2;
    pub const NEXT_SESSION: i32 = 3;
    pub const PREV_LAP: i32 = 4;
    pub const NEXT_LAP: i32 = 5;
    pub const PREV_FRAME: i32 = 6;
    pub const NEXT_FRAME: i32 = 7;
    pub const PREV_INCIDENT: i32 = 8;
    pub const NEXT_INCIDENT: i32 = 9;
    pub const LAST: i32 = 10;
}

/// `enum irsdk_RpyPosMode`
pub mod rpy_pos_mode {
    pub const BEGIN: i32 = 0;
    pub const CURRENT: i32 = 1;
    pub const END: i32 = 2;
    pub const LAST: i32 = 3;
}

/// `enum irsdk_FFBCommandMode`
pub mod ffb_command_mode {
    pub const MAX_FORCE: i32 = 0;
    pub const LAST: i32 = 1;
}

/// `enum irsdk_csMode`
pub mod cs_mode {
    pub const FOCUS_AT_INCIDENT: i32 = -3;
    pub const FOCUS_AT_LEADER: i32 = -2;
    pub const FOCUS_AT_EXITING: i32 = -1;
    pub const FOCUS_AT_DRIVER: i32 = 0;
}

/// `enum irsdk_VideoCaptureMode`
pub mod video_capture_mode {
    pub const TRIGGER_SCREEN_SHOT: i32 = 0;
    pub const START_VIDEO_CAPTURE: i32 = 1;
    pub const END_VIDEO_CAPTURE: i32 = 2;
    pub const TOGGLE_VIDEO_CAPTURE: i32 = 3;
    pub const SHOW_VIDEO_TIMER: i32 = 4;
    pub const HIDE_VIDEO_TIMER: i32 = 5;
}
