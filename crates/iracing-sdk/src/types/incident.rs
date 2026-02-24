//! Incident classification types for IRSDK IncidentFlags.

use serde::{Deserialize, Serialize};

use super::{BitField, irsdk_bitflags::IncidentFlags, irsdk_flags};

/// High-level classification of an incident as report + penalty code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentClassification {
    pub report: IncidentReport,
    pub penalty: IncidentPenalty,
}

/// Discrete incident report categories from the low byte of IncidentFlags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentReport {
    NoReport,
    OutOfControl,
    OffTrack,
    OffTrackOngoing,
    ContactWithWorld,
    CollisionWithWorld,
    CollisionWithWorldOngoing,
    ContactWithCar,
    CollisionWithCar,
    Unknown(i32),
}

impl IncidentReport {
    pub const fn from_raw(raw: i32) -> Self {
        match raw {
            0x00 => Self::NoReport,
            0x01 => Self::OutOfControl,
            0x02 => Self::OffTrack,
            0x03 => Self::OffTrackOngoing,
            0x04 => Self::ContactWithWorld,
            0x05 => Self::CollisionWithWorld,
            0x06 => Self::CollisionWithWorldOngoing,
            0x07 => Self::ContactWithCar,
            0x08 => Self::CollisionWithCar,
            other => Self::Unknown(other),
        }
    }

    pub const fn to_raw(self) -> i32 {
        match self {
            Self::NoReport => 0x00,
            Self::OutOfControl => 0x01,
            Self::OffTrack => 0x02,
            Self::OffTrackOngoing => 0x03,
            Self::ContactWithWorld => 0x04,
            Self::CollisionWithWorld => 0x05,
            Self::CollisionWithWorldOngoing => 0x06,
            Self::ContactWithCar => 0x07,
            Self::CollisionWithCar => 0x08,
            Self::Unknown(raw) => raw,
        }
    }
}

impl TryFrom<i32> for IncidentReport {
    type Error = i32;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match Self::from_raw(value) {
            Self::Unknown(raw) => Err(raw),
            known => Ok(known),
        }
    }
}

/// Discrete incident penalty magnitudes from the second byte of IncidentFlags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentPenalty {
    None,
    ZeroX,
    OneX,
    TwoX,
    FourX,
    Unknown(i32),
}

impl IncidentPenalty {
    pub const fn from_raw(raw: i32) -> Self {
        match raw {
            0x00 => Self::None,
            0x01 => Self::ZeroX,
            0x02 => Self::OneX,
            0x03 => Self::TwoX,
            0x04 => Self::FourX,
            other => Self::Unknown(other),
        }
    }

    pub const fn to_raw(self) -> i32 {
        match self {
            Self::None => 0x00,
            Self::ZeroX => 0x01,
            Self::OneX => 0x02,
            Self::TwoX => 0x03,
            Self::FourX => 0x04,
            Self::Unknown(raw) => raw,
        }
    }
}

impl TryFrom<i32> for IncidentPenalty {
    type Error = i32;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match Self::from_raw(value) {
            Self::Unknown(raw) => Err(raw),
            known => Ok(known),
        }
    }
}

/// Decode a BitField carrying `irsdk_IncidentFlags` into structured report/penalty.
pub fn decode_incident(bits: BitField) -> IncidentClassification {
    let flags = IncidentFlags::from(bits);

    IncidentClassification {
        report: IncidentReport::from_raw(flags.report_code()),
        penalty: IncidentPenalty::from_raw(flags.penalty_code()),
    }
}

/// Build an IncidentFlags bit pattern from explicit report and penalty.
pub fn encode_incident(report: IncidentReport, penalty: IncidentPenalty) -> BitField {
    let rep = (report.to_raw() as u32) & irsdk_flags::incident::REP_MASK;
    let pen = ((penalty.to_raw() as u32) << 8) & irsdk_flags::incident::PEN_MASK;
    BitField::new(rep | pen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incident_report_roundtrip_known_and_unknown() {
        let known = IncidentReport::from_raw(0x04);
        assert!(matches!(known, IncidentReport::ContactWithWorld));
        assert_eq!(known.to_raw(), 0x04);
        assert!(IncidentReport::try_from(0x04).is_ok());

        let unknown = IncidentReport::from_raw(0x7F);
        assert!(matches!(unknown, IncidentReport::Unknown(0x7F)));
        assert_eq!(unknown.to_raw(), 0x7F);
        assert!(IncidentReport::try_from(0x7F).is_err());
    }

    #[test]
    fn incident_penalty_roundtrip_known_and_unknown() {
        let known = IncidentPenalty::from_raw(0x02);
        assert!(matches!(known, IncidentPenalty::OneX));
        assert_eq!(known.to_raw(), 0x02);
        assert!(IncidentPenalty::try_from(0x02).is_ok());

        let unknown = IncidentPenalty::from_raw(0x7F);
        assert!(matches!(unknown, IncidentPenalty::Unknown(0x7F)));
        assert_eq!(unknown.to_raw(), 0x7F);
        assert!(IncidentPenalty::try_from(0x7F).is_err());
    }

    #[test]
    fn encode_decode_incident_roundtrip() {
        let raw = encode_incident(IncidentReport::CollisionWithCar, IncidentPenalty::FourX);
        let decoded = decode_incident(raw);
        assert!(matches!(decoded.report, IncidentReport::CollisionWithCar));
        assert!(matches!(decoded.penalty, IncidentPenalty::FourX));
    }
}
