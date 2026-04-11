//! Typed enum wrappers for IRSDK numeric enums.

#[cfg(feature = "codegen")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{VarData, VariableInfo};

macro_rules! define_irsdk_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident = $value:path,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[cfg_attr(feature = "codegen", derive(JsonSchema))]
        $vis enum $name {
            $(
                /// Named enum variant — see the `irsdk_flags` module for the raw constant value.
                $variant,
            )+
            /// An unrecognised value from the iRacing SDK.
            Unknown(i32),
        }

        impl $name {
            /// Constructs a typed variant from a raw `i32` value.
            ///
            /// Returns [`Self::Unknown`] for any value not covered by a named variant.
            pub const fn from_raw(raw: i32) -> Self {
                match raw {
                    $($value => Self::$variant,)+
                    other => Self::Unknown(other),
                }
            }

            /// Returns the raw `i32` representation of this variant.
            pub const fn to_raw(self) -> i32 {
                match self {
                    $(Self::$variant => $value,)+
                    Self::Unknown(v) => v,
                }
            }
        }

        #[cfg(feature = "codegen")]
        impl $name {
            /// Exhaustive list of `(variant-name, raw-value)` pairs used for JSON Schema generation.
            pub const SCHEMA_VALUES: &'static [(&'static str, i64)] = &[
                $((stringify!($variant), $value as i64),)+
            ];
        }

        impl From<$name> for u16 {
            fn from(value: $name) -> Self {
                value.to_raw() as u16
            }
        }

        impl From<$name> for u32 {
            fn from(value: $name) -> Self {
                value.to_raw() as u32
            }
        }

        impl From<$name> for i32 {
            fn from(value: $name) -> Self {
                value.to_raw()
            }
        }

        impl TryFrom<i32> for $name {
            type Error = i32;

            fn try_from(value: i32) -> Result<Self, Self::Error> {
                match Self::from_raw(value) {
                    Self::Unknown(raw) => Err(raw),
                    known => Ok(known),
                }
            }
        }

        impl VarData for $name {
            fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
                let value = <i32 as VarData>::from_bytes(data, info)?;
                Ok(Self::from_raw(value))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::from_raw(0)
            }
        }
    };
}

define_irsdk_enum! {
    /// `enum irsdk_StatusField`
    pub enum StatusField {
        Connected = super::irsdk_flags::status_field::CONNECTED,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_TrkLoc`
    pub enum TrackLocation {
        NotInWorld = super::irsdk_flags::trk_loc::NOT_IN_WORLD,
        OffTrack = super::irsdk_flags::trk_loc::OFF_TRACK,
        InPitStall = super::irsdk_flags::trk_loc::IN_PIT_STALL,
        ApproachingPits = super::irsdk_flags::trk_loc::APPROACHING_PITS,
        OnTrack = super::irsdk_flags::trk_loc::ON_TRACK,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_TrkSurf`
    pub enum TrackSurface {
        SurfaceNotInWorld = super::irsdk_flags::trk_surf::SURFACE_NOT_IN_WORLD,
        UndefinedMaterial = super::irsdk_flags::trk_surf::UNDEFINED_MATERIAL,
        Asphalt1Material = super::irsdk_flags::trk_surf::ASPHALT1_MATERIAL,
        Asphalt2Material = super::irsdk_flags::trk_surf::ASPHALT2_MATERIAL,
        Asphalt3Material = super::irsdk_flags::trk_surf::ASPHALT3_MATERIAL,
        Asphalt4Material = super::irsdk_flags::trk_surf::ASPHALT4_MATERIAL,
        Concrete1Material = super::irsdk_flags::trk_surf::CONCRETE1_MATERIAL,
        Concrete2Material = super::irsdk_flags::trk_surf::CONCRETE2_MATERIAL,
        RacingDirt1Material = super::irsdk_flags::trk_surf::RACING_DIRT1_MATERIAL,
        RacingDirt2Material = super::irsdk_flags::trk_surf::RACING_DIRT2_MATERIAL,
        Paint1Material = super::irsdk_flags::trk_surf::PAINT1_MATERIAL,
        Paint2Material = super::irsdk_flags::trk_surf::PAINT2_MATERIAL,
        Rumble1Material = super::irsdk_flags::trk_surf::RUMBLE1_MATERIAL,
        Rumble2Material = super::irsdk_flags::trk_surf::RUMBLE2_MATERIAL,
        Rumble3Material = super::irsdk_flags::trk_surf::RUMBLE3_MATERIAL,
        Rumble4Material = super::irsdk_flags::trk_surf::RUMBLE4_MATERIAL,
        Grass1Material = super::irsdk_flags::trk_surf::GRASS1_MATERIAL,
        Grass2Material = super::irsdk_flags::trk_surf::GRASS2_MATERIAL,
        Grass3Material = super::irsdk_flags::trk_surf::GRASS3_MATERIAL,
        Grass4Material = super::irsdk_flags::trk_surf::GRASS4_MATERIAL,
        Dirt1Material = super::irsdk_flags::trk_surf::DIRT1_MATERIAL,
        Dirt2Material = super::irsdk_flags::trk_surf::DIRT2_MATERIAL,
        Dirt3Material = super::irsdk_flags::trk_surf::DIRT3_MATERIAL,
        Dirt4Material = super::irsdk_flags::trk_surf::DIRT4_MATERIAL,
        SandMaterial = super::irsdk_flags::trk_surf::SAND_MATERIAL,
        Gravel1Material = super::irsdk_flags::trk_surf::GRAVEL1_MATERIAL,
        Gravel2Material = super::irsdk_flags::trk_surf::GRAVEL2_MATERIAL,
        GrasscreteMaterial = super::irsdk_flags::trk_surf::GRASSCRETE_MATERIAL,
        AstroturfMaterial = super::irsdk_flags::trk_surf::ASTROTURF_MATERIAL,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_SessionState`
    pub enum SessionState {
        Invalid = super::irsdk_flags::session_state::INVALID,
        GetInCar = super::irsdk_flags::session_state::GET_IN_CAR,
        Warmup = super::irsdk_flags::session_state::WARMUP,
        ParadeLaps = super::irsdk_flags::session_state::PARADE_LAPS,
        Racing = super::irsdk_flags::session_state::RACING,
        Checkered = super::irsdk_flags::session_state::CHECKERED,
        CoolDown = super::irsdk_flags::session_state::COOL_DOWN,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_CarLeftRight`
    pub enum CarLeftRight {
        Off = super::irsdk_flags::car_left_right::OFF,
        Clear = super::irsdk_flags::car_left_right::CLEAR,
        CarLeft = super::irsdk_flags::car_left_right::CAR_LEFT,
        CarRight = super::irsdk_flags::car_left_right::CAR_RIGHT,
        CarLeftRight = super::irsdk_flags::car_left_right::CAR_LEFT_RIGHT,
        TwoCarsLeft = super::irsdk_flags::car_left_right::TWO_CARS_LEFT,
        TwoCarsRight = super::irsdk_flags::car_left_right::TWO_CARS_RIGHT,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_PitSvStatus`
    pub enum PitServiceStatus {
        None = super::irsdk_flags::pit_sv_status::NONE,
        InProgress = super::irsdk_flags::pit_sv_status::IN_PROGRESS,
        Complete = super::irsdk_flags::pit_sv_status::COMPLETE,
        TooFarLeft = super::irsdk_flags::pit_sv_status::TOO_FAR_LEFT,
        TooFarRight = super::irsdk_flags::pit_sv_status::TOO_FAR_RIGHT,
        TooFarForward = super::irsdk_flags::pit_sv_status::TOO_FAR_FORWARD,
        TooFarBack = super::irsdk_flags::pit_sv_status::TOO_FAR_BACK,
        BadAngle = super::irsdk_flags::pit_sv_status::BAD_ANGLE,
        CantFixThat = super::irsdk_flags::pit_sv_status::CANT_FIX_THAT,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_PaceMode`
    pub enum PaceMode {
        SingleFileStart = super::irsdk_flags::pace_mode::SINGLE_FILE_START,
        DoubleFileStart = super::irsdk_flags::pace_mode::DOUBLE_FILE_START,
        SingleFileRestart = super::irsdk_flags::pace_mode::SINGLE_FILE_RESTART,
        DoubleFileRestart = super::irsdk_flags::pace_mode::DOUBLE_FILE_RESTART,
        NotPacing = super::irsdk_flags::pace_mode::NOT_PACING,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_TrackWetness`
    pub enum TrackWetness {
        UnknownWetness = super::irsdk_flags::track_wetness::UNKNOWN,
        Dry = super::irsdk_flags::track_wetness::DRY,
        MostlyDry = super::irsdk_flags::track_wetness::MOSTLY_DRY,
        VeryLightlyWet = super::irsdk_flags::track_wetness::VERY_LIGHTLY_WET,
        LightlyWet = super::irsdk_flags::track_wetness::LIGHTLY_WET,
        ModeratelyWet = super::irsdk_flags::track_wetness::MODERATELY_WET,
        VeryWet = super::irsdk_flags::track_wetness::VERY_WET,
        ExtremelyWet = super::irsdk_flags::track_wetness::EXTREMELY_WET,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_BroadcastMsg`
    pub enum BroadcastMessage {
        CamSwitchPos = super::irsdk_flags::broadcast_msg::CAM_SWITCH_POS,
        CamSwitchNum = super::irsdk_flags::broadcast_msg::CAM_SWITCH_NUM,
        CamSetState = super::irsdk_flags::broadcast_msg::CAM_SET_STATE,
        ReplaySetPlaySpeed = super::irsdk_flags::broadcast_msg::REPLAY_SET_PLAY_SPEED,
        ReplaySetPlayPosition = super::irsdk_flags::broadcast_msg::REPLAY_SET_PLAY_POSITION,
        ReplaySearch = super::irsdk_flags::broadcast_msg::REPLAY_SEARCH,
        ReplaySetState = super::irsdk_flags::broadcast_msg::REPLAY_SET_STATE,
        ReloadTextures = super::irsdk_flags::broadcast_msg::RELOAD_TEXTURES,
        ChatCommand = super::irsdk_flags::broadcast_msg::CHAT_COMMAND,
        PitCommand = super::irsdk_flags::broadcast_msg::PIT_COMMAND,
        TelemCommand = super::irsdk_flags::broadcast_msg::TELEM_COMMAND,
        FfbCommand = super::irsdk_flags::broadcast_msg::FFB_COMMAND,
        ReplaySearchSessionTime = super::irsdk_flags::broadcast_msg::REPLAY_SEARCH_SESSION_TIME,
        VideoCapture = super::irsdk_flags::broadcast_msg::VIDEO_CAPTURE,
        Last = super::irsdk_flags::broadcast_msg::LAST,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_ChatCommandMode`
    pub enum ChatCommandMode {
        Macro = super::irsdk_flags::chat_command_mode::MACRO,
        BeginChat = super::irsdk_flags::chat_command_mode::BEGIN_CHAT,
        Reply = super::irsdk_flags::chat_command_mode::REPLY,
        Cancel = super::irsdk_flags::chat_command_mode::CANCEL,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_PitCommandMode`
    pub enum PitCommandMode {
        Clear = super::irsdk_flags::pit_command_mode::CLEAR,
        Ws = super::irsdk_flags::pit_command_mode::WS,
        Fuel = super::irsdk_flags::pit_command_mode::FUEL,
        Lf = super::irsdk_flags::pit_command_mode::LF,
        Rf = super::irsdk_flags::pit_command_mode::RF,
        Lr = super::irsdk_flags::pit_command_mode::LR,
        Rr = super::irsdk_flags::pit_command_mode::RR,
        ClearTires = super::irsdk_flags::pit_command_mode::CLEAR_TIRES,
        Fr = super::irsdk_flags::pit_command_mode::FR,
        ClearWs = super::irsdk_flags::pit_command_mode::CLEAR_WS,
        ClearFr = super::irsdk_flags::pit_command_mode::CLEAR_FR,
        ClearFuel = super::irsdk_flags::pit_command_mode::CLEAR_FUEL,
        Tc = super::irsdk_flags::pit_command_mode::TC,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_TelemetryCommandMode`
    pub enum TelemetryCommandMode {
        Stop = super::irsdk_flags::telem_command_mode::STOP,
        Start = super::irsdk_flags::telem_command_mode::START,
        Restart = super::irsdk_flags::telem_command_mode::RESTART,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_RpyStateMode`
    pub enum ReplayStateMode {
        EraseTape = super::irsdk_flags::rpy_state_mode::ERASE_TAPE,
        Last = super::irsdk_flags::rpy_state_mode::LAST,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_ReloadTexturesMode`
    pub enum ReloadTexturesMode {
        All = super::irsdk_flags::reload_textures_mode::ALL,
        CarIdx = super::irsdk_flags::reload_textures_mode::CAR_IDX,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_RpySrchMode`
    pub enum ReplaySearchMode {
        ToStart = super::irsdk_flags::rpy_srch_mode::TO_START,
        ToEnd = super::irsdk_flags::rpy_srch_mode::TO_END,
        PrevSession = super::irsdk_flags::rpy_srch_mode::PREV_SESSION,
        NextSession = super::irsdk_flags::rpy_srch_mode::NEXT_SESSION,
        PrevLap = super::irsdk_flags::rpy_srch_mode::PREV_LAP,
        NextLap = super::irsdk_flags::rpy_srch_mode::NEXT_LAP,
        PrevFrame = super::irsdk_flags::rpy_srch_mode::PREV_FRAME,
        NextFrame = super::irsdk_flags::rpy_srch_mode::NEXT_FRAME,
        PrevIncident = super::irsdk_flags::rpy_srch_mode::PREV_INCIDENT,
        NextIncident = super::irsdk_flags::rpy_srch_mode::NEXT_INCIDENT,
        Last = super::irsdk_flags::rpy_srch_mode::LAST,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_RpyPosMode`
    pub enum ReplayPositionMode {
        Begin = super::irsdk_flags::rpy_pos_mode::BEGIN,
        Current = super::irsdk_flags::rpy_pos_mode::CURRENT,
        End = super::irsdk_flags::rpy_pos_mode::END,
        Last = super::irsdk_flags::rpy_pos_mode::LAST,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_FFBCommandMode`
    pub enum FfbCommandMode {
        MaxForce = super::irsdk_flags::ffb_command_mode::MAX_FORCE,
        Last = super::irsdk_flags::ffb_command_mode::LAST,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_csMode`
    pub enum CameraSwitchFocus {
        FocusAtIncident = super::irsdk_flags::cs_mode::FOCUS_AT_INCIDENT,
        FocusAtLeader = super::irsdk_flags::cs_mode::FOCUS_AT_LEADER,
        FocusAtExiting = super::irsdk_flags::cs_mode::FOCUS_AT_EXITING,
        FocusAtDriver = super::irsdk_flags::cs_mode::FOCUS_AT_DRIVER,
    }
}

define_irsdk_enum! {
    /// `enum irsdk_VideoCaptureMode`
    pub enum VideoCaptureMode {
        TriggerScreenShot = super::irsdk_flags::video_capture_mode::TRIGGER_SCREEN_SHOT,
        StartVideoCapture = super::irsdk_flags::video_capture_mode::START_VIDEO_CAPTURE,
        EndVideoCapture = super::irsdk_flags::video_capture_mode::END_VIDEO_CAPTURE,
        ToggleVideoCapture = super::irsdk_flags::video_capture_mode::TOGGLE_VIDEO_CAPTURE,
        ShowVideoTimer = super::irsdk_flags::video_capture_mode::SHOW_VIDEO_TIMER,
        HideVideoTimer = super::irsdk_flags::video_capture_mode::HIDE_VIDEO_TIMER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_enum_roundtrip {
        ($ty:ty, $known:expr, $unknown:expr) => {{
            let known = <$ty>::from_raw($known);
            assert_eq!(known.to_raw(), $known);
            assert!(<$ty>::try_from($known).is_ok());

            let unknown = <$ty>::from_raw($unknown);
            assert_eq!(unknown.to_raw(), $unknown);
            assert!(<$ty>::try_from($unknown).is_err());
        }};
    }

    #[test]
    fn typed_enum_roundtrips_and_unknowns() {
        assert_enum_roundtrip!(StatusField, 1, 77);
        assert_enum_roundtrip!(TrackLocation, 0, 77);
        assert_enum_roundtrip!(TrackSurface, 0, 99);
        assert_enum_roundtrip!(SessionState, 4, 77);
        assert_enum_roundtrip!(CarLeftRight, 1, 77);
        assert_enum_roundtrip!(PitServiceStatus, 100, 77);
        assert_enum_roundtrip!(PaceMode, 2, 77);
        assert_enum_roundtrip!(TrackWetness, 1, 77);
        assert_enum_roundtrip!(BroadcastMessage, 3, 77);
        assert_enum_roundtrip!(ChatCommandMode, 1, 77);
        assert_enum_roundtrip!(PitCommandMode, 8, 77);
        assert_enum_roundtrip!(TelemetryCommandMode, 1, 77);
        assert_enum_roundtrip!(ReplayStateMode, 1, 77);
        assert_enum_roundtrip!(ReloadTexturesMode, 1, 77);
        assert_enum_roundtrip!(ReplaySearchMode, 5, 77);
        assert_enum_roundtrip!(ReplayPositionMode, 1, 77);
        assert_enum_roundtrip!(FfbCommandMode, 0, 77);
        assert_enum_roundtrip!(CameraSwitchFocus, -1, 77);
        assert_enum_roundtrip!(VideoCaptureMode, 3, 77);
    }

    #[test]
    fn typed_enums_decode_via_vardata() {
        let data = super::super::irsdk_flags::trk_loc::ON_TRACK.to_le_bytes();
        let mut frame = vec![0u8; 8];
        frame[..4].copy_from_slice(&data);

        let info = VariableInfo {
            name: "TrackLocation".to_string(),
            data_type: VariableType::Int32,
            offset: 0,
            count: 1,
            count_as_time: false,
            units: String::new(),
            description: String::new(),
        };

        let value = TrackLocation::from_bytes(&frame, &info).expect("TrackLocation decode");
        assert!(matches!(value, TrackLocation::OnTrack));
    }

    #[test]
    fn typed_enums_have_zero_default() {
        assert_eq!(TrackLocation::default().to_raw(), 0);
    }
}
