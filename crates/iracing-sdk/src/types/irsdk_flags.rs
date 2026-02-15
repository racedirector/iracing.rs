//! Constants and helpers for interpreting IRSDK bitfields
//!
//! This module contains flag constants for EngineWarnings, SessionFlags, and IncidentFlags
//! from the iRacing SDK (IRSDK 1.19).

// Engine warnings (added in 1.19)
pub mod engine_warnings {
    pub const MAND_REP_NEEDED: u32 = 0x0080; // irsdk_mandRepNeeded
    pub const OPT_REP_NEEDED: u32 = 0x0100; // irsdk_optRepNeeded
}

// Global session flags additions (1.19)
pub mod session_flags {
    pub const DQ_SCORING_INVALID: u32 = 0x0020_0000; // irsdk_dqScoringInvalid
}

// Incident flags (1.19): combined report (low byte) + penalty (high byte)
pub mod incident {
    pub const REP_MASK: u32 = 0x0000_00FF; // IRSDK_INCIDENT_REP_MASK
    pub const PEN_MASK: u32 = 0x0000_FF00; // IRSDK_INCIDENT_PEN_MASK

    // Known report codes (low byte)
    pub const REP_NO_REPORT: u8 = 0x00;
    pub const REP_OUT_OF_CONTROL: u8 = 0x01;
    pub const REP_OFF_TRACK: u8 = 0x02;
    pub const REP_OFF_TRACK_ONGOING: u8 = 0x03; // not currently sent
    pub const REP_CONTACT_WITH_WORLD: u8 = 0x04;
    pub const REP_COLLISION_WITH_WORLD: u8 = 0x05;
    pub const REP_COLLISION_WITH_WORLD_ONGOING: u8 = 0x06; // not currently sent
    pub const REP_CONTACT_WITH_CAR: u8 = 0x07;
    pub const REP_COLLISION_WITH_CAR: u8 = 0x08;

    // Known penalty codes (second byte)
    pub const PEN_NONE: u8 = 0x00; // PenNoReport
    pub const PEN_0X: u8 = 0x01; // ZeroX
    pub const PEN_1X: u8 = 0x02; // OneX
    pub const PEN_2X: u8 = 0x03; // TwoX
    pub const PEN_4X: u8 = 0x04; // FourX
}
