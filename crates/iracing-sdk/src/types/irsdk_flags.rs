//! Low-level IRSDK constants mirrored from `irsdk_defines.h`.
//!
//! These are raw numeric values that map 1:1 with iRacing's C++ SDK symbols.
//! Prefer typed wrappers from `irsdk_enums` and `irsdk_bitflags` for application code.

/// `enum irsdk_StatusField`
pub mod status_field {
    /// The iRacing client is connected and broadcasting telemetry (`irsdk_stConnected`).
    pub const CONNECTED: i32 = 0x0000_0001;
}

/// `enum irsdk_TrkLoc`
pub mod trk_loc {
    /// Driver is not positioned in the world, e.g. viewing from the garage (`irsdk_NotInWorld`).
    pub const NOT_IN_WORLD: i32 = -1;
    /// Driver is off the racing surface (`irsdk_OffTrack`).
    pub const OFF_TRACK: i32 = 0;
    /// Driver is stopped in a pit stall (`irsdk_InPitStall`).
    pub const IN_PIT_STALL: i32 = 1;
    /// Driver is in the pit lane approaching their stall (`irsdk_AproachingPits`).
    pub const APPROACHING_PITS: i32 = 2;
    /// Driver is on the racing surface (`irsdk_OnTrack`).
    pub const ON_TRACK: i32 = 3;
}

/// `enum irsdk_TrkSurf`
pub mod trk_surf {
    /// Surface not in the world (`irsdk_SurfaceNotInWorld`).
    pub const SURFACE_NOT_IN_WORLD: i32 = -1;
    /// Undefined or unknown surface material (`irsdk_UndefinedMaterial`).
    pub const UNDEFINED_MATERIAL: i32 = 0;
    /// First asphalt surface variant.
    pub const ASPHALT1_MATERIAL: i32 = 1;
    /// Second asphalt surface variant.
    pub const ASPHALT2_MATERIAL: i32 = 2;
    /// Third asphalt surface variant.
    pub const ASPHALT3_MATERIAL: i32 = 3;
    /// Fourth asphalt surface variant.
    pub const ASPHALT4_MATERIAL: i32 = 4;
    /// First concrete surface variant.
    pub const CONCRETE1_MATERIAL: i32 = 5;
    /// Second concrete surface variant.
    pub const CONCRETE2_MATERIAL: i32 = 6;
    /// First racing dirt surface variant.
    pub const RACING_DIRT1_MATERIAL: i32 = 7;
    /// Second racing dirt surface variant.
    pub const RACING_DIRT2_MATERIAL: i32 = 8;
    /// First painted surface variant (start/finish line, etc.).
    pub const PAINT1_MATERIAL: i32 = 9;
    /// Second painted surface variant.
    pub const PAINT2_MATERIAL: i32 = 10;
    /// First rumble strip surface variant.
    pub const RUMBLE1_MATERIAL: i32 = 11;
    /// Second rumble strip surface variant.
    pub const RUMBLE2_MATERIAL: i32 = 12;
    /// Third rumble strip surface variant.
    pub const RUMBLE3_MATERIAL: i32 = 13;
    /// Fourth rumble strip surface variant.
    pub const RUMBLE4_MATERIAL: i32 = 14;
    /// First grass surface variant.
    pub const GRASS1_MATERIAL: i32 = 15;
    /// Second grass surface variant.
    pub const GRASS2_MATERIAL: i32 = 16;
    /// Third grass surface variant.
    pub const GRASS3_MATERIAL: i32 = 17;
    /// Fourth grass surface variant.
    pub const GRASS4_MATERIAL: i32 = 18;
    /// First dirt surface variant.
    pub const DIRT1_MATERIAL: i32 = 19;
    /// Second dirt surface variant.
    pub const DIRT2_MATERIAL: i32 = 20;
    /// Third dirt surface variant.
    pub const DIRT3_MATERIAL: i32 = 21;
    /// Fourth dirt surface variant.
    pub const DIRT4_MATERIAL: i32 = 22;
    /// Sand surface material.
    pub const SAND_MATERIAL: i32 = 23;
    /// First gravel surface variant.
    pub const GRAVEL1_MATERIAL: i32 = 24;
    /// Second gravel surface variant.
    pub const GRAVEL2_MATERIAL: i32 = 25;
    /// Grasscrete (grass-reinforced concrete) surface material.
    pub const GRASSCRETE_MATERIAL: i32 = 26;
    /// Artificial turf (AstroTurf) surface material.
    pub const ASTROTURF_MATERIAL: i32 = 27;
}

/// `enum irsdk_SessionState`
pub mod session_state {
    /// Session state is invalid or not yet initialised (`irsdk_StateInvalid`).
    pub const INVALID: i32 = 0;
    /// Pre-session phase: drivers are getting into their cars (`irsdk_StateGetInCar`).
    pub const GET_IN_CAR: i32 = 1;
    /// Warmup session is active (`irsdk_StateWarmup`).
    pub const WARMUP: i32 = 2;
    /// Formation / parade laps are underway (`irsdk_StateParadeLaps`).
    pub const PARADE_LAPS: i32 = 3;
    /// Racing is in progress (`irsdk_StateRacing`).
    pub const RACING: i32 = 4;
    /// Checkered flag has been shown; session is finishing (`irsdk_StateCheckered`).
    pub const CHECKERED: i32 = 5;
    /// Post-race cool-down lap (`irsdk_StateCoolDown`).
    pub const COOL_DOWN: i32 = 6;
}

/// `enum irsdk_CarLeftRight`
pub mod car_left_right {
    /// Left/right pass indicator is off (`irsdk_LROff`).
    pub const OFF: i32 = 0;
    /// No car is alongside; track is clear (`irsdk_LRClear`).
    pub const CLEAR: i32 = 1;
    /// A car is alongside on the left (`irsdk_LRCarLeft`).
    pub const CAR_LEFT: i32 = 2;
    /// A car is alongside on the right (`irsdk_LRCarRight`).
    pub const CAR_RIGHT: i32 = 3;
    /// Cars are alongside on both left and right (`irsdk_LRCarLeftRight`).
    pub const CAR_LEFT_RIGHT: i32 = 4;
    /// Two or more cars are alongside on the left (`irsdk_LR2CarsLeft`).
    pub const TWO_CARS_LEFT: i32 = 5;
    /// Two or more cars are alongside on the right (`irsdk_LR2CarsRight`).
    pub const TWO_CARS_RIGHT: i32 = 6;
}

/// `enum irsdk_PitSvStatus`
pub mod pit_sv_status {
    /// No pit service is active (`irsdk_PitSvNone`).
    pub const NONE: i32 = 0;
    /// Pit service is currently in progress (`irsdk_PitSvInProgress`).
    pub const IN_PROGRESS: i32 = 1;
    /// Pit service has completed successfully (`irsdk_PitSvComplete`).
    pub const COMPLETE: i32 = 2;
    /// Car stopped too far to the left in the stall (`irsdk_PitSvTooFarLeft`).
    pub const TOO_FAR_LEFT: i32 = 100;
    /// Car stopped too far to the right in the stall (`irsdk_PitSvTooFarRight`).
    pub const TOO_FAR_RIGHT: i32 = 101;
    /// Car stopped too far forward in the stall (`irsdk_PitSvTooFarForward`).
    pub const TOO_FAR_FORWARD: i32 = 102;
    /// Car stopped too far back in the stall (`irsdk_PitSvTooFarBack`).
    pub const TOO_FAR_BACK: i32 = 103;
    /// Car is at an unacceptable angle in the stall (`irsdk_PitSvBadAngle`).
    pub const BAD_ANGLE: i32 = 104;
    /// The requested service cannot be performed (`irsdk_PitSvCantFixThat`).
    pub const CANT_FIX_THAT: i32 = 105;
}

/// `enum irsdk_PaceMode`
pub mod pace_mode {
    /// Single-file formation for the race start (`irsdk_PaceModeSingleFileStart`).
    pub const SINGLE_FILE_START: i32 = 0;
    /// Double-file formation for the race start (`irsdk_PaceModeDoubleFileStart`).
    pub const DOUBLE_FILE_START: i32 = 1;
    /// Single-file formation for a restart (`irsdk_PaceModeSingleFileRestart`).
    pub const SINGLE_FILE_RESTART: i32 = 2;
    /// Double-file formation for a restart (`irsdk_PaceModeDoubleFileRestart`).
    pub const DOUBLE_FILE_RESTART: i32 = 3;
    /// The field is not currently pacing behind the pace car (`irsdk_PaceModeNotPacing`).
    pub const NOT_PACING: i32 = 4;
}

/// `enum irsdk_TrackWetness`
pub mod track_wetness {
    /// Track wetness is unknown (`irsdk_TrackWetness_UNKNOWN`).
    pub const UNKNOWN: i32 = 0;
    /// Track surface is dry.
    pub const DRY: i32 = 1;
    /// Track surface is mostly dry with minimal moisture.
    pub const MOSTLY_DRY: i32 = 2;
    /// Track surface has a very light film of moisture.
    pub const VERY_LIGHTLY_WET: i32 = 3;
    /// Track surface is lightly wet.
    pub const LIGHTLY_WET: i32 = 4;
    /// Track surface is moderately wet.
    pub const MODERATELY_WET: i32 = 5;
    /// Track surface is very wet.
    pub const VERY_WET: i32 = 6;
    /// Track surface is extremely wet.
    pub const EXTREMELY_WET: i32 = 7;
}

/// `enum irsdk_IncidentFlags` — packed report/penalty word.
///
/// The low byte holds the report code; the second byte holds the penalty code.
/// Use [`crate::IncidentFlags`] or the `incident` module for typed access.
pub mod incident {
    /// Mask covering the incident report code (low byte).
    pub const REP_MASK: u32 = 0x0000_00FF;
    /// Mask covering the incident penalty code (second byte).
    pub const PEN_MASK: u32 = 0x0000_FF00;

    /// Report code: no incident (`0x00`).
    pub const REP_NO_REPORT: u8 = 0x00;
    /// Report code: driver lost control (`0x01`).
    pub const REP_OUT_OF_CONTROL: u8 = 0x01;
    /// Report code: driver went off track (`0x02`).
    pub const REP_OFF_TRACK: u8 = 0x02;
    /// Report code: driver is continuing off track (`0x03`).
    pub const REP_OFF_TRACK_ONGOING: u8 = 0x03;
    /// Report code: driver made contact with a world object (`0x04`).
    pub const REP_CONTACT_WITH_WORLD: u8 = 0x04;
    /// Report code: driver collided with a world object (`0x05`).
    pub const REP_COLLISION_WITH_WORLD: u8 = 0x05;
    /// Report code: driver is in an ongoing collision with a world object (`0x06`).
    pub const REP_COLLISION_WITH_WORLD_ONGOING: u8 = 0x06;
    /// Report code: driver made contact with another car (`0x07`).
    pub const REP_CONTACT_WITH_CAR: u8 = 0x07;
    /// Report code: driver collided with another car (`0x08`).
    pub const REP_COLLISION_WITH_CAR: u8 = 0x08;

    /// Penalty code: no penalty (`0x00`).
    pub const PEN_NONE: u8 = 0x00;
    /// Penalty code: 0x (zero-multiplier) penalty (`0x01`).
    pub const PEN_0X: u8 = 0x01;
    /// Penalty code: 1x penalty (`0x02`).
    pub const PEN_1X: u8 = 0x02;
    /// Penalty code: 2x penalty (`0x03`).
    pub const PEN_2X: u8 = 0x03;
    /// Penalty code: 4x penalty (`0x04`).
    pub const PEN_4X: u8 = 0x04;
}

/// `enum irsdk_EngineWarnings`
pub mod engine_warnings {
    /// Engine coolant temperature is above the warning threshold (`0x0001`).
    pub const WATER_TEMP_WARNING: u32 = 0x0001;
    /// Fuel pressure is below the warning threshold (`0x0002`).
    pub const FUEL_PRESSURE_WARNING: u32 = 0x0002;
    /// Oil pressure is below the warning threshold (`0x0004`).
    pub const OIL_PRESSURE_WARNING: u32 = 0x0004;
    /// Engine has stalled (`0x0008`).
    pub const ENGINE_STALLED: u32 = 0x0008;
    /// Pit speed limiter is currently active (`0x0010`).
    pub const PIT_SPEED_LIMITER: u32 = 0x0010;
    /// Rev limiter is currently active (`0x0020`).
    pub const REV_LIMITER_ACTIVE: u32 = 0x0020;
    /// Engine oil temperature is above the warning threshold (`0x0040`).
    pub const OIL_TEMP_WARNING: u32 = 0x0040;
    /// Mandatory repair is needed before continuing (`0x0080`).
    pub const MAND_REP_NEEDED: u32 = 0x0080;
    /// Optional repair is available at the next pit stop (`0x0100`).
    pub const OPT_REP_NEEDED: u32 = 0x0100;
}

/// `enum irsdk_Flags` — session / race control flags.
pub mod flags {
    /// Checkered flag is being shown (`0x0000_0001`).
    pub const CHECKERED: u32 = 0x0000_0001;
    /// White flag (one lap to go) is being shown (`0x0000_0002`).
    pub const WHITE: u32 = 0x0000_0002;
    /// Green flag; racing is underway (`0x0000_0004`).
    pub const GREEN: u32 = 0x0000_0004;
    /// Yellow flag; caution period is active (`0x0000_0008`).
    pub const YELLOW: u32 = 0x0000_0008;
    /// Red flag; session is stopped (`0x0000_0010`).
    pub const RED: u32 = 0x0000_0010;
    /// Blue flag shown to a lapped car to yield (`0x0000_0020`).
    pub const BLUE: u32 = 0x0000_0020;
    /// Debris flag; hazard on the racing surface (`0x0000_0040`).
    pub const DEBRIS: u32 = 0x0000_0040;
    /// Crossed flags; mid-race point marker (`0x0000_0080`).
    pub const CROSSED: u32 = 0x0000_0080;
    /// Yellow flag is waving (`0x0000_0100`).
    pub const YELLOW_WAVING: u32 = 0x0000_0100;
    /// One lap remaining before a green flag restart (`0x0000_0200`).
    pub const ONE_LAP_TO_GREEN: u32 = 0x0000_0200;
    /// Green flag is being held; do not race yet (`0x0000_0400`).
    pub const GREEN_HELD: u32 = 0x0000_0400;
    /// Ten laps remaining in the session (`0x0000_0800`).
    pub const TEN_TO_GO: u32 = 0x0000_0800;
    /// Five laps remaining in the session (`0x0000_1000`).
    pub const FIVE_TO_GO: u32 = 0x0000_1000;
    /// Flag is waving randomly (e.g. marshal waving) (`0x0000_2000`).
    pub const RANDOM_WAVING: u32 = 0x0000_2000;
    /// Caution flag is shown and stationary (`0x0000_4000`).
    pub const CAUTION: u32 = 0x0000_4000;
    /// Caution flag is waving (`0x0000_8000`).
    pub const CAUTION_WAVING: u32 = 0x0000_8000;
    /// Black flag shown to a specific car (`0x0001_0000`).
    pub const BLACK: u32 = 0x0001_0000;
    /// Disqualification flag (`0x0002_0000`).
    pub const DISQUALIFY: u32 = 0x0002_0000;
    /// Car is serviceable (not broken) in the pit (`0x0004_0000`).
    pub const SERVICIBLE: u32 = 0x0004_0000;
    /// Furled (rolled-up) black flag shown to a car (`0x0008_0000`).
    pub const FURLED: u32 = 0x0008_0000;
    /// Repair flag; car must pit for mandatory repairs (`0x0010_0000`).
    pub const REPAIR: u32 = 0x0010_0000;
    /// Disqualified car's scoring is now invalid (`0x0020_0000`).
    pub const DQ_SCORING_INVALID: u32 = 0x0020_0000;
    /// Start light hidden; not yet displaying (`0x1000_0000`).
    pub const START_HIDDEN: u32 = 0x1000_0000;
    /// Start light shows "Ready" state (`0x2000_0000`).
    pub const START_READY: u32 = 0x2000_0000;
    /// Start light shows "Set" state (`0x4000_0000`).
    pub const START_SET: u32 = 0x4000_0000;
    /// Start light shows "Go" (green) state (`0x8000_0000`).
    pub const START_GO: u32 = 0x8000_0000;
}

/// Backward-compatible alias for legacy naming.
pub mod session_flags {
    /// Disqualified car's scoring is now invalid. Alias for [`super::flags::DQ_SCORING_INVALID`].
    pub const DQ_SCORING_INVALID: u32 = super::flags::DQ_SCORING_INVALID;
}

/// `enum irsdk_CameraState`
pub mod camera_state {
    /// The session UI / replay screen is active (`0x0001`).
    pub const IS_SESSION_SCREEN: u32 = 0x0001;
    /// Scenic (cinematic) camera is active (`0x0002`).
    pub const IS_SCENIC_ACTIVE: u32 = 0x0002;
    /// Camera tool is open and active (`0x0004`).
    pub const CAM_TOOL_ACTIVE: u32 = 0x0004;
    /// The in-game UI is hidden (`0x0008`).
    pub const UI_HIDDEN: u32 = 0x0008;
    /// Auto shot selection is enabled (`0x0010`).
    pub const USE_AUTO_SHOT_SELECTION: u32 = 0x0010;
    /// Temporary camera edits are allowed (`0x0020`).
    pub const USE_TEMPORARY_EDITS: u32 = 0x0020;
    /// Keyboard camera acceleration is active (`0x0040`).
    pub const USE_KEY_ACCELERATION: u32 = 0x0040;
    /// 10× keyboard camera acceleration is active (`0x0080`).
    pub const USE_KEY_10X_ACCELERATION: u32 = 0x0080;
    /// Mouse aim mode is active (`0x0100`).
    pub const USE_MOUSE_AIM_MODE: u32 = 0x0100;
}

/// `enum irsdk_PitSvFlags` — requested pit service operations.
pub mod pit_sv_flags {
    /// Change the left-front tyre (`0x0001`).
    pub const LF_TIRE_CHANGE: u32 = 0x0001;
    /// Change the right-front tyre (`0x0002`).
    pub const RF_TIRE_CHANGE: u32 = 0x0002;
    /// Change the left-rear tyre (`0x0004`).
    pub const LR_TIRE_CHANGE: u32 = 0x0004;
    /// Change the right-rear tyre (`0x0008`).
    pub const RR_TIRE_CHANGE: u32 = 0x0008;
    /// Fill the fuel tank (`0x0010`).
    pub const FUEL_FILL: u32 = 0x0010;
    /// Apply a windshield tear-off (`0x0020`).
    pub const WINDSHIELD_TEAROFF: u32 = 0x0020;
    /// Perform a fast repair (`0x0040`).
    pub const FAST_REPAIR: u32 = 0x0040;
}

/// `enum irsdk_PaceFlags` — per-car pace lap status flags.
pub mod pace_flags {
    /// Car is at the end of the pace line (`0x0001`).
    pub const END_OF_LINE: u32 = 0x0001;
    /// Car has been awarded a free pass (`0x0002`).
    pub const FREE_PASS: u32 = 0x0002;
    /// Car has been waved around the pace car (`0x0004`).
    pub const WAVED_AROUND: u32 = 0x0004;
}

/// `enum irsdk_BroadcastMsg` — message types for the broadcast API.
pub mod broadcast_msg {
    /// Switch camera to a car by position index (`0`).
    pub const CAM_SWITCH_POS: i32 = 0;
    /// Switch camera to a car by car number (`1`).
    pub const CAM_SWITCH_NUM: i32 = 1;
    /// Set the camera state flags (`2`).
    pub const CAM_SET_STATE: i32 = 2;
    /// Set replay playback speed (`3`).
    pub const REPLAY_SET_PLAY_SPEED: i32 = 3;
    /// Jump to a specific replay position (`4`).
    pub const REPLAY_SET_PLAY_POSITION: i32 = 4;
    /// Search for a replay event (`5`).
    pub const REPLAY_SEARCH: i32 = 5;
    /// Set the replay state (`6`).
    pub const REPLAY_SET_STATE: i32 = 6;
    /// Reload car textures (`7`).
    pub const RELOAD_TEXTURES: i32 = 7;
    /// Issue a chat command (`8`).
    pub const CHAT_COMMAND: i32 = 8;
    /// Issue a pit command (`9`).
    pub const PIT_COMMAND: i32 = 9;
    /// Issue a telemetry command (`10`).
    pub const TELEM_COMMAND: i32 = 10;
    /// Issue a force-feedback command (`11`).
    pub const FFB_COMMAND: i32 = 11;
    /// Search for a replay position by session time (`12`).
    pub const REPLAY_SEARCH_SESSION_TIME: i32 = 12;
    /// Issue a video capture command (`13`).
    pub const VIDEO_CAPTURE: i32 = 13;
    /// Sentinel value — total number of broadcast messages (`14`).
    pub const LAST: i32 = 14;
}

/// `enum irsdk_ChatCommandMode`
pub mod chat_command_mode {
    /// Execute a pre-defined chat macro by index (`0`).
    pub const MACRO: i32 = 0;
    /// Open the chat box for manual text entry (`1`).
    pub const BEGIN_CHAT: i32 = 1;
    /// Reply to the previous chat message (`2`).
    pub const REPLY: i32 = 2;
    /// Cancel the current chat action (`3`).
    pub const CANCEL: i32 = 3;
}

/// `enum irsdk_PitCommandMode`
pub mod pit_command_mode {
    /// Clear all pending pit service requests (`0`).
    pub const CLEAR: i32 = 0;
    /// Request a windshield tear-off (`1`).
    pub const WS: i32 = 1;
    /// Set the fuel request amount (litres/gallons) (`2`).
    pub const FUEL: i32 = 2;
    /// Request a left-front tyre change (`3`).
    pub const LF: i32 = 3;
    /// Request a right-front tyre change (`4`).
    pub const RF: i32 = 4;
    /// Request a left-rear tyre change (`5`).
    pub const LR: i32 = 5;
    /// Request a right-rear tyre change (`6`).
    pub const RR: i32 = 6;
    /// Clear all tyre change requests (`7`).
    pub const CLEAR_TIRES: i32 = 7;
    /// Request a fast repair (`8`).
    pub const FR: i32 = 8;
    /// Clear the windshield tear-off request (`9`).
    pub const CLEAR_WS: i32 = 9;
    /// Clear the fast repair request (`10`).
    pub const CLEAR_FR: i32 = 10;
    /// Clear the fuel request (`11`).
    pub const CLEAR_FUEL: i32 = 11;
    /// Set a tyre compound change request (`12`).
    pub const TC: i32 = 12;
}

/// `enum irsdk_TelemetryCommandMode`
pub mod telem_command_mode {
    /// Stop recording telemetry to disk (`0`).
    pub const STOP: i32 = 0;
    /// Start recording telemetry to disk (`1`).
    pub const START: i32 = 1;
    /// Restart (stop then start) telemetry recording (`2`).
    pub const RESTART: i32 = 2;
}

/// `enum irsdk_RpyStateMode`
pub mod rpy_state_mode {
    /// Erase the current replay tape (delete recorded replay data) (`0`).
    pub const ERASE_TAPE: i32 = 0;
    /// Sentinel value — total number of replay state modes (`1`).
    pub const LAST: i32 = 1;
}

/// `enum irsdk_ReloadTexturesMode`
pub mod reload_textures_mode {
    /// Reload textures for all cars (`0`).
    pub const ALL: i32 = 0;
    /// Reload textures for a specific car by index (`1`).
    pub const CAR_IDX: i32 = 1;
}

/// `enum irsdk_RpySrchMode` — replay search targets.
pub mod rpy_srch_mode {
    /// Jump to the very start of the replay (`0`).
    pub const TO_START: i32 = 0;
    /// Jump to the very end of the replay (`1`).
    pub const TO_END: i32 = 1;
    /// Jump to the previous session (`2`).
    pub const PREV_SESSION: i32 = 2;
    /// Jump to the next session (`3`).
    pub const NEXT_SESSION: i32 = 3;
    /// Jump to the previous lap (`4`).
    pub const PREV_LAP: i32 = 4;
    /// Jump to the next lap (`5`).
    pub const NEXT_LAP: i32 = 5;
    /// Step one frame backward (`6`).
    pub const PREV_FRAME: i32 = 6;
    /// Step one frame forward (`7`).
    pub const NEXT_FRAME: i32 = 7;
    /// Jump to the previous incident (`8`).
    pub const PREV_INCIDENT: i32 = 8;
    /// Jump to the next incident (`9`).
    pub const NEXT_INCIDENT: i32 = 9;
    /// Sentinel value — total number of replay search modes (`10`).
    pub const LAST: i32 = 10;
}

/// `enum irsdk_RpyPosMode` — replay position reference points.
pub mod rpy_pos_mode {
    /// Position is relative to the start of the replay (`0`).
    pub const BEGIN: i32 = 0;
    /// Position is relative to the current playback position (`1`).
    pub const CURRENT: i32 = 1;
    /// Position is relative to the end of the replay (`2`).
    pub const END: i32 = 2;
    /// Sentinel value — total number of position modes (`3`).
    pub const LAST: i32 = 3;
}

/// `enum irsdk_FFBCommandMode` — force-feedback control commands.
pub mod ffb_command_mode {
    /// Set the maximum force-feedback force level (`0`).
    pub const MAX_FORCE: i32 = 0;
    /// Sentinel value — total number of FFB command modes (`1`).
    pub const LAST: i32 = 1;
}

/// `enum irsdk_csMode` — camera switch / focus targets.
pub mod cs_mode {
    /// Focus the broadcast camera on the most recent incident (`-3`).
    pub const FOCUS_AT_INCIDENT: i32 = -3;
    /// Focus the broadcast camera on the race leader (`-2`).
    pub const FOCUS_AT_LEADER: i32 = -2;
    /// Focus the broadcast camera on the next car about to exit the pits (`-1`).
    pub const FOCUS_AT_EXITING: i32 = -1;
    /// Focus the broadcast camera on a specific driver by car index (`0` = use `var1` index) (`0`).
    pub const FOCUS_AT_DRIVER: i32 = 0;
}

/// `enum irsdk_VideoCaptureMode` — video capture commands.
pub mod video_capture_mode {
    /// Capture a single screenshot (`0`).
    pub const TRIGGER_SCREEN_SHOT: i32 = 0;
    /// Begin continuous video capture (`1`).
    pub const START_VIDEO_CAPTURE: i32 = 1;
    /// Stop continuous video capture (`2`).
    pub const END_VIDEO_CAPTURE: i32 = 2;
    /// Toggle video capture on/off (`3`).
    pub const TOGGLE_VIDEO_CAPTURE: i32 = 3;
    /// Show the on-screen video capture timer (`4`).
    pub const SHOW_VIDEO_TIMER: i32 = 4;
    /// Hide the on-screen video capture timer (`5`).
    pub const HIDE_VIDEO_TIMER: i32 = 5;
}
