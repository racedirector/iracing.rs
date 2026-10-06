//! SDK bitmask and packed-field types.
//!
//! Composite-mask predicates use `has_any_*` when one matching bit is
//! sufficient. Predicates without `any` require the complete state described
//! by their name. Use `intersects` for any matching bit and `contains` for
//! every bit in a mask.

use bitflags::bitflags;

use crate::{Result, parse_utils::read_wire_bytes};

/// `irsdk_StatusField`, stored in `irsdk_header::status` as an `int`.
#[repr(transparent)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatusField(i32);

#[cfg(feature = "debug")]
impl type_layout::TypeLayout for StatusField {
    fn type_layout() -> type_layout::TypeLayoutInfo {
        type_layout::TypeLayoutInfo {
            name: "StatusField".into(),
            size: std::mem::size_of::<Self>(),
            alignment: std::mem::align_of::<Self>(),
            fields: vec![type_layout::Field::Field {
                name: "bits".into(),
                ty: "i32".into(),
                size: std::mem::size_of::<i32>(),
            }],
        }
    }
}

bitflags! {
    impl StatusField: i32 {
        /// `irsdk_stConnected`.
        const CONNECTED = 1;
        // The source may set any bits.
        const _ = !0;
    }
}

impl StatusField {
    /// Decodes one SDK status field from its exact wire representation.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::WireSize`] if `bytes` has the wrong length.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self> {
        read_wire_bytes(bytes)
    }

    /// Helper indicating if the status field indicates "connected"
    pub fn is_connected(self) -> bool {
        self.contains(Self::CONNECTED)
    }
}

/// `irsdk_EngineWarnings`.
#[repr(transparent)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EngineWarnings(u32);

bitflags! {
    impl EngineWarnings: u32 {
        /// Water temperature warning
        const WATER_TEMP_WARNING = 0x0001;
        /// Fuel pressure warning
        const FUEL_PRESSURE_WARNING = 0x0002;
        /// Oil pressure warning
        const OIL_PRESSURE_WARNING = 0x0004;

        /// Engine stalled
        const ENGINE_STALLED = 0x0008;

        /// Pit speed limiter engaged
        const PIT_SPEED_LIMITER = 0x0010;

        /// Rev limiter engaged
        const REV_LIMITER_ACTIVE = 0x0020;

        /// Oil temperature warning
        const OIL_TEMP_WARNING = 0x0040;

        /// Engine has mandatory repairs
        const MANDATORY_REPAIR_NEEDED = 0x0080;
        /// Engine has optional repairs
        const OPTIONAL_REPAIR_NEEDED = 0x0100;

        const _ = !0;

    }
}

impl EngineWarnings {
    /// Either repair warning.
    pub const REPAIR_WARNINGS: Self =
        Self::MANDATORY_REPAIR_NEEDED.union(Self::OPTIONAL_REPAIR_NEEDED);
    /// Returns whether either repair warning is set.
    pub const fn has_any_repair_warning(self) -> bool {
        self.intersects(Self::REPAIR_WARNINGS)
    }

    /// Returns whether the mandatory-repair warning is set.
    pub const fn has_mandatory_repair_warning(self) -> bool {
        self.contains(Self::MANDATORY_REPAIR_NEEDED)
    }

    /// Returns whether the optional-repair warning is set.
    pub const fn has_optional_repair_warning(self) -> bool {
        self.contains(Self::OPTIONAL_REPAIR_NEEDED)
    }
}

/// `irsdk_Flags`.
#[repr(transparent)]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SessionFlags(u32);

bitflags! {
    impl SessionFlags: u32 {
        /// SDK mask `CHECKERED`.
        const CHECKERED = 0x0000_0001;
        /// SDK mask `WHITE`.
        const WHITE = 0x0000_0002;
        /// SDK mask `GREEN`.
        const GREEN = 0x0000_0004;
        /// SDK mask `YELLOW`.
        const YELLOW = 0x0000_0008;
        /// SDK mask `RED`.
        const RED = 0x0000_0010;
        /// SDK mask `BLUE`.
        const BLUE = 0x0000_0020;
        /// SDK mask `DEBRIS`.
        const DEBRIS = 0x0000_0040;
        /// SDK mask `CROSSED`.
        const CROSSED = 0x0000_0080;
        /// SDK mask `YELLOW_WAVING`.
        const YELLOW_WAVING = 0x0000_0100;
        /// SDK mask `ONE_LAP_TO_GREEN`.
        const ONE_LAP_TO_GREEN = 0x0000_0200;
        /// SDK mask `GREEN_HELD`.
        const GREEN_HELD = 0x0000_0400;
        /// SDK mask `TEN_TO_GO`.
        const TEN_TO_GO = 0x0000_0800;
        /// SDK mask `FIVE_TO_GO`.
        const FIVE_TO_GO = 0x0000_1000;
        /// SDK mask `RANDOM_WAVING`.
        const RANDOM_WAVING = 0x0000_2000;
        /// SDK mask `CAUTION`.
        const CAUTION = 0x0000_4000;
        /// SDK mask `CAUTION_WAVING`.
        const CAUTION_WAVING = 0x0000_8000;
        /// SDK mask `BLACK`.
        const BLACK = 0x0001_0000;
        /// SDK mask `DISQUALIFY`.
        const DISQUALIFY = 0x0002_0000;
        /// SDK mask `SERVICE_ALLOWED`.
        const SERVICE_ALLOWED = 0x0004_0000;
        /// SDK mask `FURLED`.
        const FURLED = 0x0008_0000;
        /// SDK mask `REPAIR`.
        const REPAIR = 0x0010_0000;
        /// SDK mask `DISQUALIFICATION_SCORING_INVALID`.
        const DISQUALIFICATION_SCORING_INVALID = 0x0020_0000;
        /// SDK mask `START_HIDDEN`.
        const START_HIDDEN = 0x1000_0000;
        /// SDK mask `START_READY`.
        const START_READY = 0x2000_0000;
        /// SDK mask `START_SET`.
        const START_SET = 0x4000_0000;
        /// SDK mask `START_GO`.
        const START_GO = 0x8000_0000;
        // Keep future SDK bits when decoding recorded or live values.
        const _ = !0;
    }
}

impl SessionFlags {
    /// Backward-compatible spelling of the SDK's `irsdk_servicible` flag.
    pub const SERVICIBLE: Self = Self::SERVICE_ALLOWED;

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
        self.intersects(Self::START_CONTROL_FLAGS)
    }

    /// Returns whether either caution flag is set.
    pub const fn has_any_caution(self) -> bool {
        self.intersects(Self::CAUTION_FLAGS)
    }

    /// Returns whether either yellow flag is set.
    pub const fn has_any_yellow(self) -> bool {
        self.intersects(Self::YELLOW_FLAGS)
    }

    /// Returns whether any penalty flag is set.
    pub const fn has_any_penalty(self) -> bool {
        self.intersects(Self::PENALTY_FLAGS)
    }

    /// Returns whether disqualification has invalidated scoring.
    pub const fn has_disqualification_scoring_invalid(self) -> bool {
        self.contains(Self::DISQUALIFICATION_SCORING_INVALID)
    }
}

/// `irsdk_CameraState`.
#[repr(transparent)]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CameraState(u32);

bitflags! {
    impl CameraState: u32 {
        /// SDK mask `IS_SESSION_SCREEN`.
        const IS_SESSION_SCREEN = 0x0001;
        /// SDK mask `IS_SCENIC_ACTIVE`.
        const IS_SCENIC_ACTIVE = 0x0002;
        /// SDK mask `CAMERA_TOOL_ACTIVE`.
        const CAMERA_TOOL_ACTIVE = 0x0004;
        /// SDK mask `USER_INTERFACE_HIDDEN`.
        const USER_INTERFACE_HIDDEN = 0x0008;
        /// SDK mask `USE_AUTO_SHOT_SELECTION`.
        const USE_AUTO_SHOT_SELECTION = 0x0010;
        /// SDK mask `USE_TEMPORARY_EDITS`.
        const USE_TEMPORARY_EDITS = 0x0020;
        /// SDK mask `USE_KEY_ACCELERATION`.
        const USE_KEY_ACCELERATION = 0x0040;
        /// SDK mask `USE_KEY_TEN_TIMES_ACCELERATION`.
        const USE_KEY_TEN_TIMES_ACCELERATION = 0x0080;
        /// SDK mask `USE_MOUSE_AIM_MODE`.
        const USE_MOUSE_AIM_MODE = 0x0100;
        const _ = !0;
    }
}

/// `irsdk_PitSvFlags`.
#[repr(transparent)]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PitServiceFlags(u32);

bitflags! {
    impl PitServiceFlags: u32 {
        /// SDK mask `LEFT_FRONT_TIRE_CHANGE`.
        const LEFT_FRONT_TIRE_CHANGE = 0x0001;
        /// SDK mask `RIGHT_FRONT_TIRE_CHANGE`.
        const RIGHT_FRONT_TIRE_CHANGE = 0x0002;
        /// SDK mask `LEFT_REAR_TIRE_CHANGE`.
        const LEFT_REAR_TIRE_CHANGE = 0x0004;
        /// SDK mask `RIGHT_REAR_TIRE_CHANGE`.
        const RIGHT_REAR_TIRE_CHANGE = 0x0008;
        /// SDK mask `FUEL_FILL`.
        const FUEL_FILL = 0x0010;
        /// SDK mask `WINDSHIELD_TEAROFF`.
        const WINDSHIELD_TEAROFF = 0x0020;
        /// SDK mask `FAST_REPAIR`.
        const FAST_REPAIR = 0x0040;
        const _ = !0;
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
        self.intersects(Self::TIRE_SERVICE_FLAGS)
    }

    /// Returns whether either front tire change is requested.
    pub const fn has_any_front_tire_service(self) -> bool {
        self.intersects(Self::FRONT_TIRE_SERVICE_FLAGS)
    }

    /// Returns whether either rear tire change is requested.
    pub const fn has_any_rear_tire_service(self) -> bool {
        self.intersects(Self::REAR_TIRE_SERVICE_FLAGS)
    }

    /// Returns whether either left-side tire change is requested.
    pub const fn has_any_left_side_tire_service(self) -> bool {
        self.intersects(Self::LEFT_SIDE_TIRE_SERVICE_FLAGS)
    }

    /// Returns whether either right-side tire change is requested.
    pub const fn has_any_right_side_tire_service(self) -> bool {
        self.intersects(Self::RIGHT_SIDE_TIRE_SERVICE_FLAGS)
    }

    /// Returns whether all four tires, fuel, and a tearoff are requested.
    pub const fn has_full_service(self) -> bool {
        self.contains(Self::FULL_SERVICE_FLAGS)
    }

    /// Returns whether any tire, fuel, or windshield-tearoff service is requested.
    pub const fn has_any_service(self) -> bool {
        self.intersects(Self::FULL_SERVICE_FLAGS)
    }
}

/// `irsdk_PaceFlags`.
#[repr(transparent)]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaceFlags(u32);

bitflags! {
    impl PaceFlags: u32 {
        /// SDK mask `END_OF_LINE`.
        const END_OF_LINE = 0x0001;
        /// SDK mask `FREE_PASS`.
        const FREE_PASS = 0x0002;
        /// SDK mask `WAVED_AROUND`.
        const WAVED_AROUND = 0x0004;
        const _ = !0;
    }
}

// Numeric SDK fields and the generic telemetry BitField retain all source bits.
macro_rules! impl_flag_interop {
    ($($name:ident),+ $(,)?) => {
        $(
            impl From<u32> for $name {
                fn from(bits: u32) -> Self {
                    Self::from_bits_retain(bits)
                }
            }

            impl From<$name> for u32 {
                fn from(flags: $name) -> Self {
                    flags.bits()
                }
            }

            impl From<crate::BitField> for $name {
                fn from(field: crate::BitField) -> Self {
                    Self::from_bits_retain(field.value())
                }
            }

            impl From<$name> for crate::BitField {
                fn from(flags: $name) -> Self {
                    Self::new(flags.bits())
                }
            }
        )+
    };
}

impl_flag_interop!(
    EngineWarnings,
    SessionFlags,
    CameraState,
    PitServiceFlags,
    PaceFlags
);

/// `irsdk_IncidentFlags` is two packed fields, not a set of independent flags.
#[repr(transparent)]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IncidentFlags(u32);

/// The low-byte report code in [`IncidentFlags`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IncidentReport {
    /// No report.
    NoReport,
    /// Loss of control.
    OutOfControl,
    /// Off-track report.
    OffTrack,
    /// Continuing off-track report.
    OffTrackOngoing,
    /// Contact with the world.
    ContactWithWorld,
    /// Collision with the world.
    CollisionWithWorld,
    /// Continuing collision with the world.
    CollisionWithWorldOngoing,
    /// Contact with another car.
    ContactWithCar,
    /// Collision with another car.
    CollisionWithCar,
    /// An unrecognized report code, preserved verbatim.
    Unknown(u8),
}

/// The second-byte penalty code in [`IncidentFlags`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IncidentPenalty {
    /// No penalty report (distinct from a reported zero-point penalty).
    NoReport,
    /// A zero-point penalty.
    ZeroX,
    /// A one-point penalty.
    OneX,
    /// A two-point penalty.
    TwoX,
    /// A four-point penalty.
    FourX,
    /// An unrecognized penalty code, preserved verbatim.
    Unknown(u8),
}

/// Independent report and penalty fields decoded from [`IncidentFlags`].
///
/// This does not infer severity or discard unusual combinations. Reserved upper
/// bits remain available through [`IncidentFlags::bits`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IncidentClassification {
    /// The incident report.
    pub report: IncidentReport,
    /// The reported penalty.
    pub penalty: IncidentPenalty,
}

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

    /// Decodes the report field, preserving unrecognized codes.
    pub const fn report(self) -> IncidentReport {
        match self.report_bits() {
            0 => IncidentReport::NoReport,
            1 => IncidentReport::OutOfControl,
            2 => IncidentReport::OffTrack,
            3 => IncidentReport::OffTrackOngoing,
            4 => IncidentReport::ContactWithWorld,
            5 => IncidentReport::CollisionWithWorld,
            6 => IncidentReport::CollisionWithWorldOngoing,
            7 => IncidentReport::ContactWithCar,
            8 => IncidentReport::CollisionWithCar,
            code => IncidentReport::Unknown(code),
        }
    }

    /// Decodes the penalty field independently of the report field.
    pub const fn penalty(self) -> IncidentPenalty {
        match self.penalty_bits() {
            0 => IncidentPenalty::NoReport,
            1 => IncidentPenalty::ZeroX,
            2 => IncidentPenalty::OneX,
            3 => IncidentPenalty::TwoX,
            4 => IncidentPenalty::FourX,
            code => IncidentPenalty::Unknown(code),
        }
    }

    /// Decodes both packed fields without inferring severity.
    ///
    /// ```
    /// use iracing_irsdk::{IncidentFlags, IncidentPenalty, IncidentReport};
    ///
    /// let flags = IncidentFlags::from_bits_retain(0x8000_0408);
    /// let incident = flags.classify();
    /// assert_eq!(incident.report, IncidentReport::CollisionWithCar);
    /// assert_eq!(incident.penalty, IncidentPenalty::FourX);
    /// assert_eq!(flags.bits(), 0x8000_0408);
    /// ```
    pub const fn classify(self) -> IncidentClassification {
        IncidentClassification {
            report: self.report(),
            penalty: self.penalty(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BitField;
    use zerocopy::{FromBytes, Immutable, IntoBytes};

    #[test]
    fn u32_flags_round_trip_unknown_bits() {
        fn round_trip<T: FromBytes + IntoBytes + Immutable + PartialEq + std::fmt::Debug>(
            value: T,
        ) {
            assert_eq!(T::read_from_bytes(value.as_bytes()).unwrap(), value);
        }

        const RAW: u32 = 0x8000_0408;
        round_trip(SessionFlags::from_bits_retain(RAW));
        round_trip(CameraState::from_bits_retain(RAW));
        round_trip(PitServiceFlags::from_bits_retain(RAW));
        round_trip(PaceFlags::from_bits_retain(RAW));
        round_trip(IncidentFlags::from_bits_retain(RAW));
    }

    #[test]
    fn status_field_decodes_exact_wire_bytes() {
        let status = StatusField::from_bits(StatusField::CONNECTED.bits() | 0x4000_0000).unwrap();
        let decoded = StatusField::try_from_bytes(status.as_bytes()).unwrap();

        assert_eq!(decoded, status);
        assert!(matches!(
            StatusField::try_from_bytes(&status.as_bytes()[..3]),
            Err(crate::Error::WireSize {
                expected: 4,
                actual: 3,
            })
        ));
    }

    #[test]
    fn status_field_handles_unknown_wire_bytes() {
        let status = StatusField::from_bits_retain(0x4000_0002);
        assert_eq!(
            StatusField::try_from_bytes(status.as_bytes()).unwrap(),
            status
        );
        assert_eq!(status.bits(), 0x4000_0002);
    }

    #[test]
    fn status_field_is_connected_helper() {
        let connected = StatusField::CONNECTED;
        assert!(connected.is_connected());

        let not_connected = StatusField::from_bits_retain(2);
        assert!(!not_connected.is_connected());

        let empty = StatusField::empty();
        assert!(!empty.is_connected());

        let connected_with_unknown_bits = StatusField::from_bits_retain(3);
        assert!(connected_with_unknown_bits.is_connected());
    }

    #[test]
    fn bitflags_preserve_bits_and_iterate_names() {
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
        assert_eq!(
            flags.iter_names().map(|(name, _)| name).collect::<Vec<_>>(),
            vec!["GREEN", "YELLOW"]
        );
    }

    #[test]
    fn bitflags_provide_numeric_conversions() {
        let flags = SessionFlags::from(SessionFlags::GREEN.bits());
        let bitfield = BitField::from(flags);
        assert_eq!(bitfield.value(), SessionFlags::GREEN.bits());
        assert_eq!(SessionFlags::from(bitfield), flags);
        assert_eq!(u32::from(flags), SessionFlags::GREEN.bits());
    }

    #[test]
    fn masks_preserve_unknown_bits() {
        let raw = SessionFlags::GREEN.bits() | 0x0800_0000;
        let flags = SessionFlags::from_bits(raw).unwrap();
        assert!(flags.contains(SessionFlags::GREEN));
        assert_eq!(flags.bits(), raw);
        assert_eq!(serde_json::to_string(&flags).unwrap(), raw.to_string());
        assert_eq!(
            serde_json::from_str::<SessionFlags>(&raw.to_string()).unwrap(),
            flags
        );

        let warnings = EngineWarnings::from_bits(0x8000_0000).unwrap();
        assert_eq!(warnings.bits(), 0x8000_0000);
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

        let warnings =
            EngineWarnings::MANDATORY_REPAIR_NEEDED.union(EngineWarnings::OPTIONAL_REPAIR_NEEDED);

        assert!(warnings.contains(EngineWarnings::MANDATORY_REPAIR_NEEDED));
        assert!(warnings.contains(EngineWarnings::OPTIONAL_REPAIR_NEEDED));
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
    fn incident_classification_preserves_every_field_combination() {
        let reports = [
            IncidentReport::NoReport,
            IncidentReport::OutOfControl,
            IncidentReport::OffTrack,
            IncidentReport::OffTrackOngoing,
            IncidentReport::ContactWithWorld,
            IncidentReport::CollisionWithWorld,
            IncidentReport::CollisionWithWorldOngoing,
            IncidentReport::ContactWithCar,
            IncidentReport::CollisionWithCar,
        ];
        let penalties = [
            IncidentPenalty::NoReport,
            IncidentPenalty::ZeroX,
            IncidentPenalty::OneX,
            IncidentPenalty::TwoX,
            IncidentPenalty::FourX,
        ];
        for report in 0..=u8::MAX {
            for penalty in 0..=u8::MAX {
                let raw = 0xabcd_0000 | u32::from(report) | (u32::from(penalty) << 8);
                let flags = IncidentFlags::from_bits_retain(raw);
                let expected = IncidentClassification {
                    report: reports
                        .get(usize::from(report))
                        .copied()
                        .unwrap_or(IncidentReport::Unknown(report)),
                    penalty: penalties
                        .get(usize::from(penalty))
                        .copied()
                        .unwrap_or(IncidentPenalty::Unknown(penalty)),
                };
                assert_eq!(flags.report(), expected.report);
                assert_eq!(flags.penalty(), expected.penalty);
                assert_eq!(flags.classify(), expected);
                assert_eq!(flags.bits(), raw);
            }
        }
    }

    #[test]
    fn incident_report_field_is_extracted_without_a_penalty() {
        let incident = IncidentFlags::REPORT_CONTACT_WITH_WORLD;

        assert_eq!(incident.report_bits(), 0x04);
        assert_eq!(incident.penalty_bits(), 0x00);
        assert_eq!(BitField::from(incident).value(), incident.bits());
    }

    #[test]
    fn incident_penalty_field_is_extracted_without_a_report() {
        let incident = IncidentFlags::PENALTY_ZERO_X;

        assert_eq!(incident.report_bits(), 0x00);
        assert_eq!(incident.penalty_bits(), 0x01);
    }
}
