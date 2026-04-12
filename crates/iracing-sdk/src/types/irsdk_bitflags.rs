//! Typed wrappers for IRSDK bitfield families.

#[cfg(feature = "codegen")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{BitField, VarData, VariableInfo, VariableType};

macro_rules! define_irsdk_bitflags {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $($flag:ident = $value:expr,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[cfg_attr(feature = "codegen", derive(JsonSchema))]
        $vis struct $name(u32);

        impl $name {
            $(
                /// Bitflag constant — see the `irsdk_flags` module for the corresponding raw value.
                pub const $flag: Self = Self($value);
            )+

            /// Returns the empty (zero) state with no flags set.
            pub const fn empty() -> Self {
                Self(0)
            }

            /// Constructs an instance from raw bits, retaining all set bits including unknown ones.
            pub const fn from_bits_retain(bits: u32) -> Self {
                Self(bits)
            }

            /// Returns the raw underlying `u32` bit pattern.
            pub const fn bits(self) -> u32 {
                self.0
            }

            /// Returns `true` if `self` contains all bits in `other`.
            pub const fn contains(self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }

            /// Returns `true` if `self` has any bits in common with `other`.
            pub const fn intersects(self, other: Self) -> bool {
                (self.0 & other.0) != 0
            }

            /// Returns the bitwise union (OR) of `self` and `other`.
            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }
        }

        impl $name {
            /// Bitfield definitions/associated strings
            pub const DEFINITIONS: &'static [(Self, &'static str)] = &[
                $((Self::$flag, stringify!($flag)),)+
            ];

            /// The name of all associated strings within the bitfield.
            pub fn names(self) -> Vec<&'static str> {
                Self::DEFINITIONS
                    .iter()
                    .filter_map(|(flag, name)| self.intersects(*flag).then_some(*name))
                    .collect()
            }
        }

        #[cfg(feature = "codegen")]
        impl $name {
            /// Named `(flag-name, raw-value)` pairs for all defined flags, used for JSON Schema generation.
            pub const SCHEMA_VALUES: &'static [(&'static str, i64)] = &[
                $((stringify!($flag), $value as i64),)+
            ];

            /// Bitmask that covers all defined (named) flag bits.
            pub const SCHEMA_KNOWN_MASK: u32 = 0u32 $(| ($value as u32))+;
        }

        impl From<u32> for $name {
            fn from(value: u32) -> Self {
                Self::from_bits_retain(value)
            }
        }

        impl From<$name> for u32 {
            fn from(value: $name) -> Self {
                value.bits()
            }
        }

        impl From<BitField> for $name {
            fn from(value: BitField) -> Self {
                Self::from_bits_retain(value.value())
            }
        }

        impl From<$name> for BitField {
            fn from(value: $name) -> Self {
                BitField::new(value.bits())
            }
        }

        impl VarData for $name {
            fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
                if info.data_type != VariableType::BitField {
                    return Err(crate::IRacingSDKError::TypeConversion {
                        details: format!("Expected BitField, got {:?}", info.data_type),
                    });
                }

                Ok(Self::from(BitField::from_bytes(data, info)?))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::empty()
            }
        }
    };
}

define_irsdk_bitflags! {
    /// `enum irsdk_EngineWarnings`
    pub struct EngineWarnings {
        WATER_TEMP_WARNING = super::irsdk_flags::engine_warnings::WATER_TEMP_WARNING,
        FUEL_PRESSURE_WARNING = super::irsdk_flags::engine_warnings::FUEL_PRESSURE_WARNING,
        OIL_PRESSURE_WARNING = super::irsdk_flags::engine_warnings::OIL_PRESSURE_WARNING,
        ENGINE_STALLED = super::irsdk_flags::engine_warnings::ENGINE_STALLED,
        PIT_SPEED_LIMITER = super::irsdk_flags::engine_warnings::PIT_SPEED_LIMITER,
        REV_LIMITER_ACTIVE = super::irsdk_flags::engine_warnings::REV_LIMITER_ACTIVE,
        OIL_TEMP_WARNING = super::irsdk_flags::engine_warnings::OIL_TEMP_WARNING,
        MANDATORY_REPAIR_NEEDED = super::irsdk_flags::engine_warnings::MAND_REP_NEEDED,
        OPTIONAL_REPAIR_NEEDED = super::irsdk_flags::engine_warnings::OPT_REP_NEEDED,
    }
}

impl EngineWarnings {
    /// If the engine has repairs
    pub fn has_repairs(&self) -> bool {
        self.intersects(
            EngineWarnings::MANDATORY_REPAIR_NEEDED.union(EngineWarnings::OPTIONAL_REPAIR_NEEDED),
        )
    }

    /// If the engine has required repairs
    pub fn has_required_repairs(&self) -> bool {
        self.contains(EngineWarnings::MANDATORY_REPAIR_NEEDED)
    }

    /// If the engine has optional repairs
    pub fn has_optional_repairs(&self) -> bool {
        self.contains(EngineWarnings::OPTIONAL_REPAIR_NEEDED)
    }
}

define_irsdk_bitflags! {
    /// `enum irsdk_Flags`
    pub struct SessionFlags {
        CHECKERED = super::irsdk_flags::flags::CHECKERED,
        WHITE = super::irsdk_flags::flags::WHITE,
        GREEN = super::irsdk_flags::flags::GREEN,
        YELLOW = super::irsdk_flags::flags::YELLOW,
        RED = super::irsdk_flags::flags::RED,
        BLUE = super::irsdk_flags::flags::BLUE,
        DEBRIS = super::irsdk_flags::flags::DEBRIS,
        CROSSED = super::irsdk_flags::flags::CROSSED,
        YELLOW_WAVING = super::irsdk_flags::flags::YELLOW_WAVING,
        ONE_LAP_TO_GREEN = super::irsdk_flags::flags::ONE_LAP_TO_GREEN,
        GREEN_HELD = super::irsdk_flags::flags::GREEN_HELD,
        TEN_TO_GO = super::irsdk_flags::flags::TEN_TO_GO,
        FIVE_TO_GO = super::irsdk_flags::flags::FIVE_TO_GO,
        RANDOM_WAVING = super::irsdk_flags::flags::RANDOM_WAVING,
        CAUTION = super::irsdk_flags::flags::CAUTION,
        CAUTION_WAVING = super::irsdk_flags::flags::CAUTION_WAVING,
        BLACK = super::irsdk_flags::flags::BLACK,
        DISQUALIFY = super::irsdk_flags::flags::DISQUALIFY,
        SERVICIBLE = super::irsdk_flags::flags::SERVICIBLE,
        FURLED = super::irsdk_flags::flags::FURLED,
        REPAIR = super::irsdk_flags::flags::REPAIR,
        DQ_SCORING_INVALID = super::irsdk_flags::flags::DQ_SCORING_INVALID,
        START_HIDDEN = super::irsdk_flags::flags::START_HIDDEN,
        START_READY = super::irsdk_flags::flags::START_READY,
        START_SET = super::irsdk_flags::flags::START_SET,
        START_GO = super::irsdk_flags::flags::START_GO,
    }
}

impl SessionFlags {
    /// Bitfield representing start control being shown.
    pub const START_CONTROL_FLAGS: Self = SessionFlags::START_HIDDEN
        .union(SessionFlags::START_READY)
        .union(SessionFlags::START_SET)
        .union(SessionFlags::START_GO);

    /// Bitfield representing any race control flag being shown.
    pub const RACE_CONTROL_FLAGS: Self = SessionFlags::CHECKERED
        .union(SessionFlags::WHITE)
        .union(SessionFlags::GREEN)
        .union(SessionFlags::GREEN_HELD)
        .union(SessionFlags::ONE_LAP_TO_GREEN)
        .union(SessionFlags::YELLOW)
        .union(SessionFlags::YELLOW_WAVING)
        .union(SessionFlags::CAUTION)
        .union(SessionFlags::CAUTION_WAVING)
        .union(SessionFlags::DEBRIS)
        .union(SessionFlags::CROSSED)
        .union(SessionFlags::FURLED)
        .union(SessionFlags::BLACK)
        .union(SessionFlags::RED)
        .union(SessionFlags::BLUE);

    /// Bitfield representing any caution being shown.
    pub const CAUTION_FLAGS: Self = SessionFlags::CAUTION.union(SessionFlags::CAUTION_WAVING);

    /// Bitfield representing any yellow being shown.
    pub const YELLOW_FLAGS: Self = SessionFlags::YELLOW.union(SessionFlags::YELLOW_WAVING);

    /// Indicates start control being shown.
    pub fn has_start_control(&self) -> bool {
        self.intersects(Self::START_CONTROL_FLAGS)
    }

    /// Indicates whether the session is under caution.
    pub fn has_caution(&self) -> bool {
        self.intersects(Self::CAUTION_FLAGS)
    }

    /// Indicates whether the session is yellow.
    pub fn has_yellow(&self) -> bool {
        self.intersects(Self::YELLOW_FLAGS)
    }

    /// Indicates whether scoring has been invalidated.
    pub fn has_dq_scoring_invalid(&self) -> bool {
        self.contains(SessionFlags::DQ_SCORING_INVALID)
    }
}

define_irsdk_bitflags! {
    /// `enum irsdk_CameraState`
    pub struct CameraState {
        IS_SESSION_SCREEN = super::irsdk_flags::camera_state::IS_SESSION_SCREEN,
        IS_SCENIC_ACTIVE = super::irsdk_flags::camera_state::IS_SCENIC_ACTIVE,
        CAM_TOOL_ACTIVE = super::irsdk_flags::camera_state::CAM_TOOL_ACTIVE,
        UI_HIDDEN = super::irsdk_flags::camera_state::UI_HIDDEN,
        USE_AUTO_SHOT_SELECTION = super::irsdk_flags::camera_state::USE_AUTO_SHOT_SELECTION,
        USE_TEMPORARY_EDITS = super::irsdk_flags::camera_state::USE_TEMPORARY_EDITS,
        USE_KEY_ACCELERATION = super::irsdk_flags::camera_state::USE_KEY_ACCELERATION,
        USE_KEY_10X_ACCELERATION = super::irsdk_flags::camera_state::USE_KEY_10X_ACCELERATION,
        USE_MOUSE_AIM_MODE = super::irsdk_flags::camera_state::USE_MOUSE_AIM_MODE,
    }
}

define_irsdk_bitflags! {
    /// `enum irsdk_PitSvFlags`
    pub struct PitServiceFlags {
        LF_TIRE_CHANGE = super::irsdk_flags::pit_sv_flags::LF_TIRE_CHANGE,
        RF_TIRE_CHANGE = super::irsdk_flags::pit_sv_flags::RF_TIRE_CHANGE,
        LR_TIRE_CHANGE = super::irsdk_flags::pit_sv_flags::LR_TIRE_CHANGE,
        RR_TIRE_CHANGE = super::irsdk_flags::pit_sv_flags::RR_TIRE_CHANGE,
        FUEL_FILL = super::irsdk_flags::pit_sv_flags::FUEL_FILL,
        WINDSHIELD_TEAROFF = super::irsdk_flags::pit_sv_flags::WINDSHIELD_TEAROFF,
        FAST_REPAIR = super::irsdk_flags::pit_sv_flags::FAST_REPAIR,
    }
}

impl PitServiceFlags {
    /// Any tire service flag
    pub const TIRE_SERVICE: Self = PitServiceFlags::LF_TIRE_CHANGE
        .union(PitServiceFlags::RF_TIRE_CHANGE)
        .union(PitServiceFlags::LR_TIRE_CHANGE)
        .union(PitServiceFlags::RR_TIRE_CHANGE);

    /// Whether the service request incldues any tire service.
    pub fn has_tire_service(&self) -> bool {
        self.intersects(Self::TIRE_SERVICE)
    }

    /// If the next stop is full service. Full service is all 4 tires, fuel, and a tearoff.
    pub fn has_full_service(&self) -> bool {
        self.intersects(
            Self::TIRE_SERVICE
                .union(PitServiceFlags::FUEL_FILL)
                .union(PitServiceFlags::WINDSHIELD_TEAROFF),
        )
    }
}

define_irsdk_bitflags! {
    /// `enum irsdk_PaceFlags`
    pub struct PaceFlags {
        END_OF_LINE = super::irsdk_flags::pace_flags::END_OF_LINE,
        FREE_PASS = super::irsdk_flags::pace_flags::FREE_PASS,
        WAVED_AROUND = super::irsdk_flags::pace_flags::WAVED_AROUND,
    }
}

/// `enum irsdk_IncidentFlags` as a combined report+penalty container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "codegen", derive(JsonSchema))]
pub struct IncidentFlags(u32);

impl IncidentFlags {
    /// Bitmask covering the report code portion of an incident flag word (low byte, `0x0000_00FF`).
    pub const REP_MASK: u32 = super::irsdk_flags::incident::REP_MASK;
    /// Bitmask covering the penalty code portion of an incident flag word (second byte, `0x0000_FF00`).
    pub const PEN_MASK: u32 = super::irsdk_flags::incident::PEN_MASK;

    #[cfg(feature = "codegen")]
    /// Named `(code-name, raw-value)` pairs for incident report codes, used for JSON Schema generation.
    pub const SCHEMA_REPORT_CODES: &'static [(&'static str, i64)] = &[
        (
            "REP_NO_REPORT",
            super::irsdk_flags::incident::REP_NO_REPORT as i64,
        ),
        (
            "REP_OUT_OF_CONTROL",
            super::irsdk_flags::incident::REP_OUT_OF_CONTROL as i64,
        ),
        (
            "REP_OFF_TRACK",
            super::irsdk_flags::incident::REP_OFF_TRACK as i64,
        ),
        (
            "REP_OFF_TRACK_ONGOING",
            super::irsdk_flags::incident::REP_OFF_TRACK_ONGOING as i64,
        ),
        (
            "REP_CONTACT_WITH_WORLD",
            super::irsdk_flags::incident::REP_CONTACT_WITH_WORLD as i64,
        ),
        (
            "REP_COLLISION_WITH_WORLD",
            super::irsdk_flags::incident::REP_COLLISION_WITH_WORLD as i64,
        ),
        (
            "REP_COLLISION_WITH_WORLD_ONGOING",
            super::irsdk_flags::incident::REP_COLLISION_WITH_WORLD_ONGOING as i64,
        ),
        (
            "REP_CONTACT_WITH_CAR",
            super::irsdk_flags::incident::REP_CONTACT_WITH_CAR as i64,
        ),
        (
            "REP_COLLISION_WITH_CAR",
            super::irsdk_flags::incident::REP_COLLISION_WITH_CAR as i64,
        ),
    ];

    #[cfg(feature = "codegen")]
    /// Named `(code-name, raw-value)` pairs for incident penalty codes, used for JSON Schema generation.
    pub const SCHEMA_PENALTY_CODES: &'static [(&'static str, i64)] = &[
        ("PEN_NONE", super::irsdk_flags::incident::PEN_NONE as i64),
        ("PEN_0X", super::irsdk_flags::incident::PEN_0X as i64),
        ("PEN_1X", super::irsdk_flags::incident::PEN_1X as i64),
        ("PEN_2X", super::irsdk_flags::incident::PEN_2X as i64),
        ("PEN_4X", super::irsdk_flags::incident::PEN_4X as i64),
    ];

    /// Constructs an `IncidentFlags` value from raw bits, retaining all set bits.
    pub const fn from_bits_retain(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the raw `u32` bit pattern.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Extracts the incident report code from the low byte (`REP_MASK`).
    pub const fn report_code(self) -> i32 {
        (self.0 & Self::REP_MASK) as i32
    }

    /// Extracts the penalty code from the second byte (`PEN_MASK >> 8`).
    pub const fn penalty_code(self) -> i32 {
        ((self.0 & Self::PEN_MASK) >> 8) as i32
    }
}

impl From<u32> for IncidentFlags {
    fn from(value: u32) -> Self {
        Self::from_bits_retain(value)
    }
}

impl From<IncidentFlags> for u32 {
    fn from(value: IncidentFlags) -> Self {
        value.bits()
    }
}

impl From<BitField> for IncidentFlags {
    fn from(value: BitField) -> Self {
        Self::from_bits_retain(value.value())
    }
}

impl From<IncidentFlags> for BitField {
    /// Converts an `IncidentFlags` value into a `BitField`.
    ///
    /// # Examples
    ///
    /// ```
    /// use iracing_sdk::{BitField, IncidentFlags};
    ///
    /// let flags = IncidentFlags::from_bits_retain(0x_00_04_03); // arbitrary raw bits
    /// let bf = BitField::from(flags);
    /// assert_eq!(bf.value(), flags.bits());
    /// ```
    fn from(value: IncidentFlags) -> Self {
        BitField::new(value.bits())
    }
}

impl Default for IncidentFlags {
    /// Create an empty bitflags value with no bits set.
    ///
    /// # Examples
    ///
    /// ```
    /// use iracing_sdk::IncidentFlags;
    ///
    /// let flags = IncidentFlags::default();
    /// assert_eq!(flags.bits(), 0);
    /// ```
    fn default() -> Self {
        Self::from_bits_retain(0)
    }
}

impl VarData for IncidentFlags {
    /// Decode this bitflag type from a byte slice using the provided VariableInfo.
    ///
    /// Validates that `info.data_type` is `VariableType::BitField`; if not, returns a
    /// `TypeConversion` error. On success, decodes a `BitField` from `data` and converts it into `Self`,
    /// propagating any decoding errors from `BitField::from_bytes`.
    ///
    /// # Returns
    ///
    /// `Self` decoded from the provided bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use iracing_sdk::{SessionFlags, VarData, VariableInfo, VariableType};
    ///
    /// let mut frame = [0u8; 8];
    /// frame[..4].copy_from_slice(&SessionFlags::GREEN.bits().to_le_bytes());
    ///
    /// let info = VariableInfo {
    ///     name: "SessionFlags".to_string(),
    ///     data_type: VariableType::BitField,
    ///     offset: 0,
    ///     count: 1,
    ///     count_as_time: false,
    ///     units: String::new(),
    ///     description: "Session flags".to_string(),
    /// };
    ///
    /// let val = SessionFlags::from_bytes(&frame, &info).unwrap();
    /// assert!(val.contains(SessionFlags::GREEN));
    /// ```
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if info.data_type != VariableType::BitField {
            return Err(crate::IRacingSDKError::TypeConversion {
                details: format!("Expected BitField, got {:?}", info.data_type),
            });
        }

        Ok(Self::from(BitField::from_bytes(data, info)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitflag_family_operations() {
        let engine = EngineWarnings::from_bits_retain(
            EngineWarnings::WATER_TEMP_WARNING.bits()
                | EngineWarnings::MANDATORY_REPAIR_NEEDED.bits(),
        );
        assert!(engine.contains(EngineWarnings::WATER_TEMP_WARNING));
        assert!(engine.intersects(EngineWarnings::MANDATORY_REPAIR_NEEDED));
        assert!(!engine.contains(EngineWarnings::OPTIONAL_REPAIR_NEEDED));

        let session = SessionFlags::from_bits_retain(
            SessionFlags::GREEN.bits() | SessionFlags::DQ_SCORING_INVALID.bits(),
        );
        assert!(session.contains(SessionFlags::GREEN));
        assert!(session.contains(SessionFlags::DQ_SCORING_INVALID));

        let camera = CameraState::from_bits_retain(CameraState::UI_HIDDEN.bits());
        assert!(camera.contains(CameraState::UI_HIDDEN));

        let pit = PitServiceFlags::from_bits_retain(PitServiceFlags::FAST_REPAIR.bits());
        assert!(pit.contains(PitServiceFlags::FAST_REPAIR));

        let pace = PaceFlags::from_bits_retain(PaceFlags::FREE_PASS.bits());
        assert!(pace.contains(PaceFlags::FREE_PASS));
    }

    #[test]
    fn incident_flags_extract_codes() {
        let bits = super::super::irsdk_flags::incident::REP_COLLISION_WITH_CAR as u32
            | ((super::super::irsdk_flags::incident::PEN_4X as u32) << 8);
        let incident = IncidentFlags::from_bits_retain(bits);
        assert_eq!(incident.report_code(), 0x08);
        assert_eq!(incident.penalty_code(), 0x04);
    }

    #[test]
    fn bitflags_decode_via_vardata() {
        let mut frame = vec![0u8; 8];
        frame[..4].copy_from_slice(&SessionFlags::GREEN.bits().to_le_bytes());

        let info = VariableInfo {
            name: "SessionFlags".to_string(),
            data_type: VariableType::BitField,
            offset: 0,
            count: 1,
            count_as_time: false,
            units: String::new(),
            description: String::new(),
        };

        let value = SessionFlags::from_bytes(&frame, &info).expect("SessionFlags decode");
        assert!(value.contains(SessionFlags::GREEN));
    }

    #[test]
    fn incident_flags_default_is_empty() {
        assert_eq!(IncidentFlags::default().bits(), 0);
    }

    #[test]
    fn bitflags_definitions_and_names_only_include_known_bits() {
        assert_eq!(
            PaceFlags::DEFINITIONS,
            &[
                (PaceFlags::END_OF_LINE, "END_OF_LINE"),
                (PaceFlags::FREE_PASS, "FREE_PASS"),
                (PaceFlags::WAVED_AROUND, "WAVED_AROUND"),
            ]
        );

        assert!(PaceFlags::empty().names().is_empty());
        assert!(PaceFlags::from_bits_retain(0x8000_0000).names().is_empty());

        let flags = PaceFlags::END_OF_LINE.union(PaceFlags::WAVED_AROUND);
        assert_eq!(flags.names(), vec!["END_OF_LINE", "WAVED_AROUND"]);
    }

    #[test]
    fn engine_warnings_repairs_helpers() {
        let none = EngineWarnings::empty();
        assert!(!none.has_repairs());
        assert!(!none.has_required_repairs());
        assert!(!none.has_optional_repairs());

        let mandatory = EngineWarnings::MANDATORY_REPAIR_NEEDED;
        assert!(mandatory.has_repairs());
        assert!(mandatory.has_required_repairs());
        assert!(!mandatory.has_optional_repairs());

        let optional = EngineWarnings::OPTIONAL_REPAIR_NEEDED;
        assert!(optional.has_repairs());
        assert!(!optional.has_required_repairs());
        assert!(optional.has_optional_repairs());

        let both = mandatory.union(optional);
        assert!(both.has_repairs());
        assert!(both.has_required_repairs());
        assert!(both.has_optional_repairs());
    }

    #[test]
    fn session_flags_helpers() {
        assert!(SessionFlags::START_CONTROL_FLAGS.contains(SessionFlags::START_HIDDEN));
        assert!(SessionFlags::START_CONTROL_FLAGS.contains(SessionFlags::START_READY));
        assert!(SessionFlags::START_CONTROL_FLAGS.contains(SessionFlags::START_SET));
        assert!(SessionFlags::START_CONTROL_FLAGS.contains(SessionFlags::START_GO));

        let none = SessionFlags::empty();
        assert!(!none.has_start_control());
        assert!(!none.has_caution());
        assert!(!none.has_yellow());
        assert!(!none.has_dq_scoring_invalid());

        let start = SessionFlags::START_READY;
        assert!(start.has_start_control());
        assert!(!start.has_caution());
        assert!(!start.has_yellow());

        let caution = SessionFlags::CAUTION;
        assert!(!caution.has_start_control());
        assert!(caution.has_caution());
        assert!(!caution.has_yellow());

        let yellow = SessionFlags::YELLOW_WAVING;
        assert!(!yellow.has_start_control());
        assert!(!yellow.has_caution());
        assert!(yellow.has_yellow());

        let dq = SessionFlags::DQ_SCORING_INVALID;
        assert!(dq.has_dq_scoring_invalid());
        assert!(!none.union(SessionFlags::GREEN).has_dq_scoring_invalid());
    }

    #[test]
    fn pit_service_flags_helpers() {
        assert!(PitServiceFlags::TIRE_SERVICE.contains(PitServiceFlags::LF_TIRE_CHANGE));
        assert!(PitServiceFlags::TIRE_SERVICE.contains(PitServiceFlags::RF_TIRE_CHANGE));
        assert!(PitServiceFlags::TIRE_SERVICE.contains(PitServiceFlags::LR_TIRE_CHANGE));
        assert!(PitServiceFlags::TIRE_SERVICE.contains(PitServiceFlags::RR_TIRE_CHANGE));

        let none = PitServiceFlags::empty();
        assert!(!none.has_tire_service());

        let lf = PitServiceFlags::LF_TIRE_CHANGE;
        assert!(lf.has_tire_service());

        let fuel_only = PitServiceFlags::FUEL_FILL;
        assert!(!fuel_only.has_tire_service());

        let mixed = PitServiceFlags::FUEL_FILL.union(PitServiceFlags::RR_TIRE_CHANGE);
        assert!(mixed.has_tire_service());
    }
}
