//! SDK bitmask and packed-field types.

use super::macros::sdk_bitmask;
use type_layout::TypeLayout;

/// `irsdk_StatusField`, stored in `irsdk_header::status` as an `int`.
#[repr(transparent)]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    TypeLayout,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "codegen", derive(schemars::JsonSchema))]
pub struct StatusField {
    bits: i32,
}

impl StatusField {
    /// `irsdk_stConnected`.
    pub const CONNECTED: Self = Self { bits: 1 };

    /// Constructs the status field without discarding unknown bits.
    pub const fn from_bits(bits: i32) -> Self {
        Self { bits }
    }

    /// Returns the complete underlying SDK bit pattern.
    pub const fn bits(self) -> i32 {
        self.bits
    }

    /// Returns whether all bits in `other` are set.
    pub const fn contains(self, other: Self) -> bool {
        self.bits & other.bits == other.bits
    }

    /// Returns whether the SDK's connected status bit is set.
    pub fn is_connected(self) -> bool {
        self.contains(Self::CONNECTED)
    }
}

sdk_bitmask! {
    /// `irsdk_EngineWarnings`.
    pub struct EngineWarnings {
        WATER_TEMP_WARNING = 0x0001,
        FUEL_PRESSURE_WARNING = 0x0002,
        OIL_PRESSURE_WARNING = 0x0004,
        ENGINE_STALLED = 0x0008,
        PIT_SPEED_LIMITER = 0x0010,
        REV_LIMITER_ACTIVE = 0x0020,
        OIL_TEMP_WARNING = 0x0040,
        MANDATORY_REPAIR_NEEDED = 0x0080,
        OPTIONAL_REPAIR_NEEDED = 0x0100,
    }
}

impl EngineWarnings {
    /// Repair warnings from the engine bitfield.
    pub const REPAIR_WARNINGS: Self =
        Self::MANDATORY_REPAIR_NEEDED.union(Self::OPTIONAL_REPAIR_NEEDED);

    /// If the engine has repairs
    pub fn has_repairs(&self) -> bool {
        self.intersects(Self::REPAIR_WARNINGS)
    }

    /// If the engine has required repairs
    pub fn has_required_repairs(&self) -> bool {
        self.contains(Self::MANDATORY_REPAIR_NEEDED)
    }

    /// If the engine has optional repairs
    pub fn has_optional_repairs(&self) -> bool {
        self.contains(Self::OPTIONAL_REPAIR_NEEDED)
    }
}

sdk_bitmask! {
    /// `irsdk_Flags`.
    pub struct SessionFlags {
        CHECKERED = 0x0000_0001,
        WHITE = 0x0000_0002,
        GREEN = 0x0000_0004,
        YELLOW = 0x0000_0008,
        RED = 0x0000_0010,
        BLUE = 0x0000_0020,
        DEBRIS = 0x0000_0040,
        CROSSED = 0x0000_0080,
        YELLOW_WAVING = 0x0000_0100,
        ONE_LAP_TO_GREEN = 0x0000_0200,
        GREEN_HELD = 0x0000_0400,
        TEN_TO_GO = 0x0000_0800,
        FIVE_TO_GO = 0x0000_1000,
        RANDOM_WAVING = 0x0000_2000,
        CAUTION = 0x0000_4000,
        CAUTION_WAVING = 0x0000_8000,
        BLACK = 0x0001_0000,
        DISQUALIFY = 0x0002_0000,
        SERVICE_ALLOWED = 0x0004_0000,
        FURLED = 0x0008_0000,
        REPAIR = 0x0010_0000,
        DISQUALIFICATION_SCORING_INVALID = 0x0020_0000,
        START_HIDDEN = 0x1000_0000,
        START_READY = 0x2000_0000,
        START_SET = 0x4000_0000,
        START_GO = 0x8000_0000,
    }
}

impl SessionFlags {
    /// Bitfield representing penalty flags
    pub const PENALTY_FLAGS: Self = Self::BLACK
        .union(Self::DISQUALIFY)
        .union(Self::FURLED)
        .union(Self::DISQUALIFICATION_SCORING_INVALID);

    /// Bitfield representing start control being shown.
    /// Excludes `START_HIDDEN`.
    pub const START_CONTROL_FLAGS: Self = Self::START_READY
        .union(Self::START_SET)
        .union(Self::START_GO);

    /// Bitfield representing any race control flag being shown.
    pub const RACE_CONTROL_FLAGS: Self = Self::CHECKERED
        .union(Self::WHITE)
        .union(Self::GREEN)
        .union(Self::GREEN_HELD)
        .union(Self::ONE_LAP_TO_GREEN)
        .union(Self::YELLOW)
        .union(Self::YELLOW_WAVING)
        .union(Self::CAUTION)
        .union(Self::CAUTION_WAVING)
        .union(Self::DEBRIS)
        .union(Self::CROSSED)
        .union(Self::FURLED)
        .union(Self::BLACK)
        .union(Self::RED)
        .union(Self::BLUE);

    /// Bitfield representing any caution being shown.
    pub const CAUTION_FLAGS: Self = Self::CAUTION.union(Self::CAUTION_WAVING);

    /// Bitfield representing any yellow being shown.
    pub const YELLOW_FLAGS: Self = Self::YELLOW.union(Self::YELLOW_WAVING);

    /// Flags tht are shown over a range
    pub const RANGE_FLAGS: Self = Self::YELLOW_FLAGS
        .union(Self::BLUE)
        .union(Self::DEBRIS)
        .union(Self::CROSSED)
        .union(Self::CAUTION_FLAGS)
        .union(Self::BLACK)
        .union(Self::SERVICE_ALLOWED)
        .union(Self::FURLED)
        .union(Self::REPAIR);

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

    /// Indicates whether the car has a penalty
    pub fn has_penalty(&self) -> bool {
        self.intersects(Self::PENALTY_FLAGS)
    }

    /// Indicates whether scoring has been invalidated.
    pub fn has_dq_scoring_invalid(&self) -> bool {
        self.contains(Self::DISQUALIFICATION_SCORING_INVALID)
    }
}

sdk_bitmask! {
    /// `irsdk_CameraState`.
    pub struct CameraState {
        IS_SESSION_SCREEN = 0x0001,
        IS_SCENIC_ACTIVE = 0x0002,
        CAMERA_TOOL_ACTIVE = 0x0004,
        USER_INTERFACE_HIDDEN = 0x0008,
        USE_AUTO_SHOT_SELECTION = 0x0010,
        USE_TEMPORARY_EDITS = 0x0020,
        USE_KEY_ACCELERATION = 0x0040,
        USE_KEY_TEN_TIMES_ACCELERATION = 0x0080,
        USE_MOUSE_AIM_MODE = 0x0100,
    }
}

sdk_bitmask! {
    /// `irsdk_PitSvFlags`.
    pub struct PitServiceFlags {
        LEFT_FRONT_TIRE_CHANGE = 0x0001,
        RIGHT_FRONT_TIRE_CHANGE = 0x0002,
        LEFT_REAR_TIRE_CHANGE = 0x0004,
        RIGHT_REAR_TIRE_CHANGE = 0x0008,
        FUEL_FILL = 0x0010,
        WINDSHIELD_TEAROFF = 0x0020,
        FAST_REPAIR = 0x0040,
    }
}

impl PitServiceFlags {
    /// Any tire service flag
    pub const TIRE_SERVICE: Self = Self::LEFT_FRONT_TIRE_CHANGE
        .union(Self::RIGHT_FRONT_TIRE_CHANGE)
        .union(Self::LEFT_REAR_TIRE_CHANGE)
        .union(Self::RIGHT_REAR_TIRE_CHANGE);

    /// Whether the service request incldues any tire service.
    pub fn has_tire_service(&self) -> bool {
        self.intersects(Self::TIRE_SERVICE)
    }

    /// Whether the service request includes front tire changes.
    pub fn has_front_tire_service(&self) -> bool {
        self.intersects(Self::LEFT_FRONT_TIRE_CHANGE.union(Self::RIGHT_FRONT_TIRE_CHANGE))
    }

    /// Whether the service request includes rear tire changes.
    pub fn has_rear_tire_service(&self) -> bool {
        self.intersects(Self::LEFT_REAR_TIRE_CHANGE.union(Self::RIGHT_REAR_TIRE_CHANGE))
    }

    /// Whether the service request includes left side tire changes.
    pub fn has_left_side_tire_service(&self) -> bool {
        self.intersects(Self::LEFT_FRONT_TIRE_CHANGE.union(Self::LEFT_REAR_TIRE_CHANGE))
    }

    /// Whether the service request includes right side tire changes.
    pub fn has_right_side_tire_service(&self) -> bool {
        self.intersects(Self::RIGHT_FRONT_TIRE_CHANGE.union(Self::RIGHT_REAR_TIRE_CHANGE))
    }

    /// If the next stop is full service. Full service is all 4 tires, fuel, and a tearoff.
    pub fn has_full_service(&self) -> bool {
        self.intersects(
            Self::TIRE_SERVICE
                .union(Self::FUEL_FILL)
                .union(Self::WINDSHIELD_TEAROFF),
        )
    }
}

sdk_bitmask! {
    /// `irsdk_PaceFlags`.
    pub struct PaceFlags {
        END_OF_LINE = 0x0001,
        FREE_PASS = 0x0002,
        WAVED_AROUND = 0x0004,
    }
}

/// `irsdk_IncidentFlags` is two packed fields, not a set of independent flags.
#[repr(transparent)]
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "codegen", derive(schemars::JsonSchema))]
pub struct IncidentFlags(u32);

impl IncidentFlags {
    /// `irsdk_Incident_RepNoReport`.
    pub const REPORT_NONE: Self = Self(0x0000);
    /// `irsdk_Incident_RepOutOfControl`.
    pub const REPORT_OUT_OF_CONTROL: Self = Self(0x0001);
    /// `irsdk_Incident_RepOffTrack`.
    pub const REPORT_OFF_TRACK: Self = Self(0x0002);
    /// `irsdk_Incident_RepOffTrackOngoing`.
    pub const REPORT_OFF_TRACK_ONGOING: Self = Self(0x0003);
    /// `irsdk_Incident_RepContactWithWorld`.
    pub const REPORT_CONTACT_WITH_WORLD: Self = Self(0x0004);
    /// `irsdk_Incident_RepCollisionWithWorld`.
    pub const REPORT_COLLISION_WITH_WORLD: Self = Self(0x0005);
    /// `irsdk_Incident_RepCollisionWithWorldOngoing`.
    pub const REPORT_COLLISION_WITH_WORLD_ONGOING: Self = Self(0x0006);
    /// `irsdk_Incident_RepContactWithCar`.
    pub const REPORT_CONTACT_WITH_CAR: Self = Self(0x0007);
    /// `irsdk_Incident_RepCollisionWithCar`.
    pub const REPORT_COLLISION_WITH_CAR: Self = Self(0x0008);

    /// `irsdk_Incident_PenNoReport`.
    pub const PENALTY_NONE: Self = Self(0x0000);
    /// `irsdk_Incident_PenZeroX`.
    pub const PENALTY_ZERO_X: Self = Self(0x0100);
    /// `irsdk_Incident_PenOneX`.
    pub const PENALTY_ONE_X: Self = Self(0x0200);
    /// `irsdk_Incident_PenTwoX`.
    pub const PENALTY_TWO_X: Self = Self(0x0300);
    /// `irsdk_Incident_PenFourX`.
    pub const PENALTY_FOUR_X: Self = Self(0x0400);

    /// `IRSDK_INCIDENT_REP_MASK`.
    pub const REPORT_MASK: u32 = 0x0000_00ff;
    /// `IRSDK_INCIDENT_PEN_MASK`.
    pub const PENALTY_MASK: u32 = 0x0000_ff00;

    /// Constructs the packed SDK value without interpreting its fields.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the complete packed SDK value.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns the low-byte incident report field.
    pub const fn report_bits(self) -> u8 {
        (self.0 & Self::REPORT_MASK) as u8
    }

    /// Returns the second-byte incident penalty field.
    pub const fn penalty_bits(self) -> u8 {
        ((self.0 & Self::PENALTY_MASK) >> 8) as u8
    }
}

impl From<crate::BitField> for IncidentFlags {
    fn from(value: crate::BitField) -> Self {
        Self::from_bits(value.value())
    }
}

impl From<IncidentFlags> for crate::BitField {
    fn from(value: IncidentFlags) -> Self {
        Self::new(value.bits())
    }
}

impl crate::VarData for IncidentFlags {
    fn from_bytes(data: &[u8], info: &crate::VariableInfo) -> crate::Result<Self> {
        <crate::BitField as crate::VarData>::from_bytes(data, info).map(Self::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BitField, VarData, VariableInfo, VariableType};

    #[test]
    fn bitmask_macro_provides_the_existing_structural_api() {
        let flags = SessionFlags::empty()
            .union(SessionFlags::GREEN)
            .union(SessionFlags::YELLOW);

        assert_eq!(
            flags.bits(),
            SessionFlags::GREEN.bits() | SessionFlags::YELLOW.bits()
        );
        assert_eq!(
            SessionFlags::from_bits_retain(flags.bits() | 0x0800_0000).bits(),
            flags.bits() | 0x0800_0000
        );
        assert_eq!(flags.names(), vec!["GREEN", "YELLOW"]);
        assert!(SessionFlags::DEFINITIONS.contains(&(SessionFlags::GREEN, "GREEN")));
    }

    #[test]
    fn bitmask_macro_provides_numeric_and_telemetry_conversions() {
        let flags = SessionFlags::from(SessionFlags::GREEN.bits());
        let bitfield = BitField::from(flags);
        assert_eq!(bitfield.value(), SessionFlags::GREEN.bits());
        assert_eq!(SessionFlags::from(bitfield), flags);
        assert_eq!(u32::from(flags), SessionFlags::GREEN.bits());

        let info = VariableInfo {
            name: "SessionFlags".to_owned(),
            data_type: VariableType::BitField,
            offset: 0,
            count: 1,
            count_as_time: false,
            units: String::new(),
            description: String::new(),
        };
        let decoded = SessionFlags::from_bytes(&SessionFlags::GREEN.bits().to_le_bytes(), &info)
            .expect("decode SDK bitmask");
        assert_eq!(decoded, SessionFlags::GREEN);
    }

    #[cfg(feature = "codegen")]
    #[test]
    fn bitmask_macro_provides_schema_metadata() {
        assert!(SessionFlags::SCHEMA_VALUES.contains(&("GREEN", 0x0000_0004)));
        assert_eq!(
            SessionFlags::SCHEMA_KNOWN_MASK & SessionFlags::GREEN.bits(),
            SessionFlags::GREEN.bits()
        );
        let _ = schemars::schema_for!(SessionFlags);
    }

    #[test]
    fn masks_preserve_unknown_bits() {
        let flags = SessionFlags::from_bits(SessionFlags::GREEN.bits() | 0x0800_0000);
        assert!(flags.contains(SessionFlags::GREEN));
        assert_eq!(flags.bits(), 0x0800_0004);
    }

    #[test]
    fn incident_fields_are_extracted_independently() {
        let incident = IncidentFlags::from_bits(
            IncidentFlags::REPORT_COLLISION_WITH_CAR.bits() | IncidentFlags::PENALTY_FOUR_X.bits(),
        );
        assert_eq!(incident.report_bits(), 8);
        assert_eq!(incident.penalty_bits(), 4);
    }
}
