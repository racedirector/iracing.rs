//! Typed wrappers for IRSDK bitfield families.

use serde::{Deserialize, Serialize};

use super::BitField;

macro_rules! define_irsdk_bitflags {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $($flag:ident = $value:expr,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        $vis struct $name(u32);

        impl $name {
            $(pub const $flag: Self = Self($value);)+

            pub const fn empty() -> Self {
                Self(0)
            }

            pub const fn from_bits_retain(bits: u32) -> Self {
                Self(bits)
            }

            pub const fn bits(self) -> u32 {
                self.0
            }

            pub const fn contains(self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }

            pub const fn intersects(self, other: Self) -> bool {
                (self.0 & other.0) != 0
            }

            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }
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
pub struct IncidentFlags(u32);

impl IncidentFlags {
    pub const REP_MASK: u32 = super::irsdk_flags::incident::REP_MASK;
    pub const PEN_MASK: u32 = super::irsdk_flags::incident::PEN_MASK;

    pub const fn from_bits_retain(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn report_code(self) -> i32 {
        (self.0 & Self::REP_MASK) as i32
    }

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
    fn from(value: IncidentFlags) -> Self {
        BitField::new(value.bits())
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
}
