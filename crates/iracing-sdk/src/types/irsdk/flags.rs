//! SDK bitmask and packed-field types.
//!
//! Composite-mask predicates use `has_any_*` when one matching bit is
//! sufficient. Predicates without `any` require the complete state described
//! by their name. At the structural level, [`EngineWarnings::has_any`] and the
//! corresponding methods on each bitmask test for any matching bit, while
//! `has_all` tests for every bit in a mask.

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

    /// Represents the disconnected/empty state.
    pub const fn empty() -> Self {
        Self::from_bits(0)
    }

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

    /// Returns whether either repair warning is set.
    pub const fn has_any_repair_warning(self) -> bool {
        self.has_any(Self::REPAIR_WARNINGS)
    }

    /// Returns whether the mandatory-repair warning is set.
    pub const fn has_mandatory_repair_warning(self) -> bool {
        self.has_all(Self::MANDATORY_REPAIR_NEEDED)
    }

    /// Returns whether the optional-repair warning is set.
    pub const fn has_optional_repair_warning(self) -> bool {
        self.has_all(Self::OPTIONAL_REPAIR_NEEDED)
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

    /// Returns whether any visible start-control flag is set.
    pub const fn has_any_start_control(self) -> bool {
        self.has_any(Self::START_CONTROL_FLAGS)
    }

    /// Returns whether either caution flag is set.
    pub const fn has_any_caution(self) -> bool {
        self.has_any(Self::CAUTION_FLAGS)
    }

    /// Returns whether either yellow flag is set.
    pub const fn has_any_yellow(self) -> bool {
        self.has_any(Self::YELLOW_FLAGS)
    }

    /// Returns whether any penalty flag is set.
    pub const fn has_any_penalty(self) -> bool {
        self.has_any(Self::PENALTY_FLAGS)
    }

    /// Returns whether disqualification has invalidated scoring.
    pub const fn has_disqualification_scoring_invalid(self) -> bool {
        self.has_all(Self::DISQUALIFICATION_SCORING_INVALID)
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
    /// All tire-change service flags.
    pub const TIRE_SERVICE_FLAGS: Self = Self::LEFT_FRONT_TIRE_CHANGE
        .union(Self::RIGHT_FRONT_TIRE_CHANGE)
        .union(Self::LEFT_REAR_TIRE_CHANGE)
        .union(Self::RIGHT_REAR_TIRE_CHANGE);

    /// Both front tire-change service flags.
    pub const FRONT_TIRE_SERVICE_FLAGS: Self =
        Self::LEFT_FRONT_TIRE_CHANGE.union(Self::RIGHT_FRONT_TIRE_CHANGE);

    /// Both rear tire-change service flags.
    pub const REAR_TIRE_SERVICE_FLAGS: Self =
        Self::LEFT_REAR_TIRE_CHANGE.union(Self::RIGHT_REAR_TIRE_CHANGE);

    /// Both left-side tire-change service flags.
    pub const LEFT_SIDE_TIRE_SERVICE_FLAGS: Self =
        Self::LEFT_FRONT_TIRE_CHANGE.union(Self::LEFT_REAR_TIRE_CHANGE);

    /// Both right-side tire-change service flags.
    pub const RIGHT_SIDE_TIRE_SERVICE_FLAGS: Self =
        Self::RIGHT_FRONT_TIRE_CHANGE.union(Self::RIGHT_REAR_TIRE_CHANGE);

    /// Every service flag required for a full stop.
    pub const FULL_SERVICE_FLAGS: Self = Self::TIRE_SERVICE_FLAGS
        .union(Self::FUEL_FILL)
        .union(Self::WINDSHIELD_TEAROFF);

    /// Returns whether any tire-change service is requested.
    pub const fn has_any_tire_service(self) -> bool {
        self.has_any(Self::TIRE_SERVICE_FLAGS)
    }

    /// Returns whether either front tire change is requested.
    pub const fn has_any_front_tire_service(self) -> bool {
        self.has_any(Self::FRONT_TIRE_SERVICE_FLAGS)
    }

    /// Returns whether either rear tire change is requested.
    pub const fn has_any_rear_tire_service(self) -> bool {
        self.has_any(Self::REAR_TIRE_SERVICE_FLAGS)
    }

    /// Returns whether either left-side tire change is requested.
    pub const fn has_any_left_side_tire_service(self) -> bool {
        self.has_any(Self::LEFT_SIDE_TIRE_SERVICE_FLAGS)
    }

    /// Returns whether either right-side tire change is requested.
    pub const fn has_any_right_side_tire_service(self) -> bool {
        self.has_any(Self::RIGHT_SIDE_TIRE_SERVICE_FLAGS)
    }

    /// Returns whether all four tires, fuel, and a tearoff are requested.
    pub const fn has_full_service(self) -> bool {
        self.has_all(Self::FULL_SERVICE_FLAGS)
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

    #[cfg(feature = "codegen")]
    /// Named incident-report codes used for schema generation.
    pub const SCHEMA_REPORT_CODES: &'static [(&'static str, i64)] = &[
        ("REP_NO_REPORT", 0x00),
        ("REP_OUT_OF_CONTROL", 0x01),
        ("REP_OFF_TRACK", 0x02),
        ("REP_OFF_TRACK_ONGOING", 0x03),
        ("REP_CONTACT_WITH_WORLD", 0x04),
        ("REP_COLLISION_WITH_WORLD", 0x05),
        ("REP_COLLISION_WITH_WORLD_ONGOING", 0x06),
        ("REP_CONTACT_WITH_CAR", 0x07),
        ("REP_COLLISION_WITH_CAR", 0x08),
    ];

    #[cfg(feature = "codegen")]
    /// Named incident-penalty codes used for schema generation.
    pub const SCHEMA_PENALTY_CODES: &'static [(&'static str, i64)] = &[
        ("PEN_NONE", 0x00),
        ("PEN_0X", 0x01),
        ("PEN_1X", 0x02),
        ("PEN_2X", 0x03),
        ("PEN_4X", 0x04),
    ];

    /// Constructs the packed SDK value without interpreting its fields.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Constructs the packed SDK value while retaining every supplied bit.
    pub const fn from_bits_retain(bits: u32) -> Self {
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
        match info.data_type {
            crate::VariableType::BitField => {
                <crate::BitField as crate::VarData>::from_bytes(data, info).map(Self::from)
            }
            crate::VariableType::Int32 => <i32 as crate::VarData>::from_bytes(data, info)
                .map(|value| Self::from(value as u32)),
            actual => Err(crate::IRacingSDKError::type_conversion(
                "BitField or Int32",
                actual,
            )),
        }
    }
}

#[cfg(feature = "codegen")]
impl schemars::JsonSchema for IncidentFlags {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "IncidentFlags".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        concat!(module_path!(), "::IncidentFlags").into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        #[allow(dead_code)]
        #[derive(schemars::JsonSchema)]
        struct SchemaRepresentation(u32);

        let mut schema = SchemaRepresentation::json_schema(generator);
        let schema_object = schema.ensure_object();
        schema_object.insert("x-irsdk-kind".into(), "incident-flags".into());

        let mut masks = serde_json::Map::new();
        masks.insert("report".into(), (Self::REPORT_MASK as u64).into());
        masks.insert("penalty".into(), (Self::PENALTY_MASK as u64).into());
        schema_object.insert("x-irsdk-masks".into(), serde_json::Value::Object(masks));
        schema_object.insert(
            "x-irsdk-report-codes".into(),
            crate::types::codegen::named_schema_values(Self::SCHEMA_REPORT_CODES),
        );
        schema_object.insert(
            "x-irsdk-penalty-codes".into(),
            crate::types::codegen::named_schema_values(Self::SCHEMA_PENALTY_CODES),
        );
        schema
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BitField, VarData, VariableInfo, VariableType};

    fn variable_info(data_type: VariableType) -> VariableInfo {
        VariableInfo {
            name: "PlayerIncidents".to_owned(),
            data_type,
            offset: 0,
            count: 1,
            count_as_time: false,
            units: String::new(),
            description: String::new(),
        }
    }

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
    fn engine_repair_predicates_distinguish_any_from_specific_warnings() {
        let mandatory = EngineWarnings::MANDATORY_REPAIR_NEEDED;
        assert!(mandatory.has_any_repair_warning());
        assert!(mandatory.has_mandatory_repair_warning());
        assert!(!mandatory.has_optional_repair_warning());

        let optional = EngineWarnings::OPTIONAL_REPAIR_NEEDED;
        assert!(optional.has_any_repair_warning());
        assert!(!optional.has_mandatory_repair_warning());
        assert!(optional.has_optional_repair_warning());

        assert!(!EngineWarnings::empty().has_any_repair_warning());
    }

    #[test]
    fn session_group_predicates_match_any_member() {
        assert!(SessionFlags::START_READY.has_any_start_control());
        assert!(SessionFlags::CAUTION_WAVING.has_any_caution());
        assert!(SessionFlags::YELLOW_WAVING.has_any_yellow());
        assert!(SessionFlags::BLACK.has_any_penalty());
        assert!(
            SessionFlags::DISQUALIFICATION_SCORING_INVALID.has_disqualification_scoring_invalid()
        );

        let unrelated = SessionFlags::GREEN;
        assert!(!unrelated.has_any_start_control());
        assert!(!unrelated.has_any_caution());
        assert!(!unrelated.has_any_yellow());
        assert!(!unrelated.has_any_penalty());
        assert!(!unrelated.has_disqualification_scoring_invalid());
    }

    #[test]
    fn full_pit_service_requires_every_required_service() {
        let one_tire = PitServiceFlags::LEFT_FRONT_TIRE_CHANGE;
        assert!(one_tire.has_any_tire_service());
        assert!(one_tire.has_any_front_tire_service());
        assert!(one_tire.has_any_left_side_tire_service());
        assert!(!one_tire.has_any_rear_tire_service());
        assert!(!one_tire.has_any_right_side_tire_service());
        assert!(!one_tire.has_full_service());

        assert!(!PitServiceFlags::TIRE_SERVICE_FLAGS.has_full_service());
        assert!(
            !PitServiceFlags::TIRE_SERVICE_FLAGS
                .union(PitServiceFlags::FUEL_FILL)
                .has_full_service()
        );

        assert!(PitServiceFlags::FULL_SERVICE_FLAGS.has_full_service());
        assert!(
            PitServiceFlags::FULL_SERVICE_FLAGS
                .union(PitServiceFlags::FAST_REPAIR)
                .has_full_service()
        );
    }

    #[test]
    fn incident_fields_are_extracted_independently() {
        let incident = IncidentFlags::from_bits(
            IncidentFlags::REPORT_COLLISION_WITH_CAR.bits() | IncidentFlags::PENALTY_FOUR_X.bits(),
        );
        assert_eq!(incident.report_bits(), 8);
        assert_eq!(incident.penalty_bits(), 4);
    }

    #[test]
    fn incident_flags_preserve_raw_numeric_conversions() {
        const RAW: u32 = 0x8000_0408;

        let incident = IncidentFlags::from(RAW);
        assert_eq!(incident, IncidentFlags::from_bits_retain(RAW));
        assert_eq!(u32::from(incident), RAW);
        assert_eq!(BitField::from(incident).value(), RAW);
        assert_eq!(IncidentFlags::from(BitField::new(RAW)), incident);
    }

    #[test]
    fn incident_flags_decode_bitfield_and_int32_storage() {
        const RAW: u32 = 0x8000_0408;

        let from_bitfield =
            IncidentFlags::from_bytes(&RAW.to_le_bytes(), &variable_info(VariableType::BitField))
                .expect("decode IncidentFlags from BitField storage");
        let from_int32 = IncidentFlags::from_bytes(
            &(RAW as i32).to_le_bytes(),
            &variable_info(VariableType::Int32),
        )
        .expect("decode IncidentFlags from Int32 storage");

        assert_eq!(from_bitfield.bits(), RAW);
        assert_eq!(from_int32.bits(), RAW);
        assert_eq!(from_int32.report_bits(), 8);
        assert_eq!(from_int32.penalty_bits(), 4);
    }

    #[test]
    fn incident_flags_reject_other_storage_types() {
        let error =
            IncidentFlags::from_bytes(&0u32.to_le_bytes(), &variable_info(VariableType::UInt32))
                .expect_err("UInt32 must not decode as IncidentFlags");

        assert!(matches!(
            error,
            crate::IRacingSDKError::TypeConversion { .. }
        ));
        assert!(error.to_string().contains("BitField or Int32"));
    }

    #[test]
    fn adapter_validation_accepts_incident_storage_types() {
        for data_type in [VariableType::BitField, VariableType::Int32] {
            assert_eq!(
                crate::adapters::telemetry_type_mismatch_details::<IncidentFlags>(&variable_info(
                    data_type
                ))
                .expect("probe IncidentFlags compatibility"),
                None
            );
        }
    }

    #[cfg(feature = "codegen")]
    #[test]
    fn incident_schema_describes_both_packed_fields() {
        use serde_json::Value;

        let schema = schemars::schema_for!(IncidentFlags);
        let object = schema
            .as_value()
            .as_object()
            .expect("incident schema should be an object");

        assert_eq!(
            object.get("x-irsdk-kind").and_then(Value::as_str),
            Some("incident-flags")
        );
        assert_eq!(
            object["x-irsdk-masks"]["report"].as_u64(),
            Some(IncidentFlags::REPORT_MASK as u64)
        );
        assert_eq!(
            object["x-irsdk-masks"]["penalty"].as_u64(),
            Some(IncidentFlags::PENALTY_MASK as u64)
        );
        assert_eq!(
            object["x-irsdk-report-codes"].as_array().map(Vec::len),
            Some(IncidentFlags::SCHEMA_REPORT_CODES.len())
        );
        assert_eq!(
            object["x-irsdk-penalty-codes"].as_array().map(Vec::len),
            Some(IncidentFlags::SCHEMA_PENALTY_CODES.len())
        );
    }
}
