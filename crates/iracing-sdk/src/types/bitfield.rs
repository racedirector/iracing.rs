//! BitField type for handling iRacing bitfield variables

use serde::{Deserialize, Serialize};

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
    let warnings = super::irsdk_bitflags::EngineWarnings::from(bits);
    warnings.has_required_repairs()
}

/// Convenience: check if EngineWarnings indicate optional repair needed (1.19)
pub fn engine_optional_repair_needed(bits: BitField) -> bool {
    let warnings = super::irsdk_bitflags::EngineWarnings::from(bits);
    warnings.has_optional_repairs()
}

/// Convenience: check if EngineWarnings include any repair needed (1.19)
pub fn engine_repairs_needed(bits: BitField) -> bool {
    let warnings = super::irsdk_bitflags::EngineWarnings::from(bits);
    warnings.has_repairs()
}

/// Convenience: check if SessionFlags indicate disqualification scoring invalid (1.19)
pub fn session_dq_scoring_invalid(flags: BitField) -> bool {
    let session_flags = super::irsdk_bitflags::SessionFlags::from(flags);
    session_flags.has_dq_scoring_invalid()
}

/// Convenience: check if SessionFlags indicate start control being shown (1.19)
pub fn session_start_control_shown(flags: BitField) -> bool {
    let session_flags = super::irsdk_bitflags::SessionFlags::from(flags);
    session_flags.has_start_control()
}

/// Convenience: check if SessionFlags indicate penalty being shown
pub fn session_penalty_shown(flags: BitField) -> bool {
    let session_flags = super::irsdk_bitflags::SessionFlags::from(flags);
    session_flags.has_penalty()
}

/// Convenience: check if SessionFlags indicate the session is under caution (1.19)
pub fn session_under_caution(flags: BitField) -> bool {
    let session_flags = super::irsdk_bitflags::SessionFlags::from(flags);
    session_flags.has_caution()
}

/// Convenience: check if SessionFlags indicate the session is yellow (1.19)
pub fn session_under_yellow(flags: BitField) -> bool {
    let session_flags = super::irsdk_bitflags::SessionFlags::from(flags);
    session_flags.has_yellow()
}

/// Convenience: check if PitServiceFlags include any tire service request (1.19)
pub fn pit_service_has_tire_service(flags: BitField) -> bool {
    let pit_service = super::irsdk_bitflags::PitServiceFlags::from(flags);
    pit_service.has_tire_service()
}

/// Convenience: check if PitServiceFlags represent "full service" (4 tires + fuel + tearoff) (1.19)
pub fn pit_service_has_full_service(flags: BitField) -> bool {
    let pit_service = super::irsdk_bitflags::PitServiceFlags::from(flags);
    pit_service.has_full_service()
}
