//! BitField type for handling iRacing bitfield variables

use serde::{Deserialize, Serialize};

use crate::{EngineWarnings, PitServiceFlags, SessionFlags};

/// BitField type for handling iRacing bitfield variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitField(pub u32);

impl BitField {
    /// Create a new BitField from a u32 value.
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Check if a specific bit is set.
    pub fn is_set(&self, bit: u32) -> bool {
        (self.0 & (1 << bit)) != 0
    }

    /// Check if a specific flag is set using a bitmask.
    pub fn has_flag(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    /// Get the raw u32 value.
    pub fn value(&self) -> u32 {
        self.0
    }
}

/// Compare tick counters in u32 space with wraparound using half-range rule.
/// Returns true if `a` is considered newer than `b`.
pub fn tick_after_u32(a: u32, b: u32) -> bool {
    if a == b {
        return false;
    }
    a.wrapping_sub(b) < 0x8000_0000
}

/// Convenience: check if EngineWarnings indicate mandatory repair needed (1.19)
pub fn engine_mandatory_repair_needed(bits: BitField) -> bool {
    let warnings = EngineWarnings::from(bits);
    warnings.has_mandatory_repair_warning()
}

/// Convenience: check if EngineWarnings indicate optional repair needed (1.19)
pub fn engine_optional_repair_needed(bits: BitField) -> bool {
    let warnings = EngineWarnings::from(bits);
    warnings.has_optional_repair_warning()
}

/// Convenience: check if EngineWarnings include any repair needed (1.19)
pub fn engine_repairs_needed(bits: BitField) -> bool {
    let warnings = EngineWarnings::from(bits);
    warnings.has_any_repair_warning()
}

/// Convenience: check if SessionFlags indicate disqualification scoring invalid (1.19)
pub fn session_dq_scoring_invalid(flags: BitField) -> bool {
    let session_flags = SessionFlags::from(flags);
    session_flags.has_disqualification_scoring_invalid()
}

/// Convenience: check if SessionFlags indicate start control being shown (1.19)
pub fn session_start_control_shown(flags: BitField) -> bool {
    let session_flags = SessionFlags::from(flags);
    session_flags.has_any_start_control()
}

/// Convenience: check if SessionFlags indicate penalty being shown
pub fn session_penalty_shown(flags: BitField) -> bool {
    let session_flags = SessionFlags::from(flags);
    session_flags.has_any_penalty()
}

/// Convenience: check if SessionFlags indicate the session is under caution (1.19)
pub fn session_under_caution(flags: BitField) -> bool {
    let session_flags = SessionFlags::from(flags);
    session_flags.has_any_caution()
}

/// Convenience: check if SessionFlags indicate the session is yellow (1.19)
pub fn session_under_yellow(flags: BitField) -> bool {
    let session_flags = SessionFlags::from(flags);
    session_flags.has_any_yellow()
}

/// Convenience: check if PitServiceFlags include any tire service request (1.19)
pub fn pit_service_has_tire_service(flags: BitField) -> bool {
    let pit_service = PitServiceFlags::from(flags);
    pit_service.has_any_tire_service()
}

/// Convenience: check if PitServiceFlags represent "full service" (4 tires + fuel + tearoff) (1.19)
pub fn pit_service_has_full_service(flags: BitField) -> bool {
    let pit_service = PitServiceFlags::from(flags);
    pit_service.has_full_service()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_repair_helpers_accept_typed_warning_bits() {
        let flags = BitField::from(
            EngineWarnings::MANDATORY_REPAIR_NEEDED.union(EngineWarnings::OPTIONAL_REPAIR_NEEDED),
        );
        assert!(engine_mandatory_repair_needed(flags));
        assert!(engine_optional_repair_needed(flags));
        assert!(engine_repairs_needed(flags));

        let none = BitField::from(EngineWarnings::empty());
        assert!(!engine_mandatory_repair_needed(none));
        assert!(!engine_optional_repair_needed(none));
        assert!(!engine_repairs_needed(none));
    }

    #[test]
    fn session_dq_scoring_invalid_helper_accepts_typed_flags() {
        let flags = BitField::from(SessionFlags::DISQUALIFICATION_SCORING_INVALID);
        assert!(session_dq_scoring_invalid(flags));

        let none = BitField::from(SessionFlags::empty());
        assert!(!session_dq_scoring_invalid(none));
    }

    #[test]
    fn session_control_and_caution_helpers_accept_typed_flags() {
        let none = BitField::from(SessionFlags::empty());
        assert!(!session_start_control_shown(none));
        assert!(!session_under_caution(none));
        assert!(!session_under_yellow(none));

        let start = BitField::from(SessionFlags::START_READY);
        assert!(session_start_control_shown(start));
        assert!(!session_under_caution(start));
        assert!(!session_under_yellow(start));

        let caution = BitField::from(SessionFlags::CAUTION_WAVING);
        assert!(!session_start_control_shown(caution));
        assert!(session_under_caution(caution));
        assert!(!session_under_yellow(caution));

        let yellow = BitField::from(SessionFlags::YELLOW_WAVING);
        assert!(!session_start_control_shown(yellow));
        assert!(!session_under_caution(yellow));
        assert!(session_under_yellow(yellow));
    }

    #[test]
    fn pit_service_helpers_accept_typed_flags() {
        let none = BitField::from(PitServiceFlags::empty());
        assert!(!pit_service_has_tire_service(none));
        assert!(!pit_service_has_full_service(none));

        let tire_only = BitField::from(PitServiceFlags::RIGHT_REAR_TIRE_CHANGE);
        assert!(pit_service_has_tire_service(tire_only));
        assert!(!pit_service_has_full_service(tire_only));

        let fuel_only = BitField::from(PitServiceFlags::FUEL_FILL);
        assert!(!pit_service_has_tire_service(fuel_only));
        assert!(!pit_service_has_full_service(fuel_only));

        let full_service = BitField::from(PitServiceFlags::FULL_SERVICE_FLAGS);
        assert!(pit_service_has_tire_service(full_service));
        assert!(pit_service_has_full_service(full_service));
    }
}
