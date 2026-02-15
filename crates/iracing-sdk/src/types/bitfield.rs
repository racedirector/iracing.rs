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
    bits.has_flag(super::irsdk_flags::engine_warnings::MAND_REP_NEEDED)
}

/// Convenience: check if EngineWarnings indicate optional repair needed (1.19)
pub fn engine_optional_repair_needed(bits: BitField) -> bool {
    bits.has_flag(super::irsdk_flags::engine_warnings::OPT_REP_NEEDED)
}

/// Convenience: check if SessionFlags indicate disqualification scoring invalid (1.19)
pub fn session_dq_scoring_invalid(flags: BitField) -> bool {
    flags.has_flag(super::irsdk_flags::session_flags::DQ_SCORING_INVALID)
}
