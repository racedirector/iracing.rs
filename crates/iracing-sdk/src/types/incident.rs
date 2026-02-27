//! Incident classification types for IRSDK IncidentFlags.

use serde::{Deserialize, Serialize};

use super::{BitField, irsdk_bitflags::IncidentFlags, irsdk_flags};

/// High-level classification of an incident as report + penalty code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentClassification {
    /// The incident report category decoded from the low byte of `IncidentFlags`.
    pub report: IncidentReport,
    /// The penalty multiplier decoded from the second byte of `IncidentFlags`.
    pub penalty: IncidentPenalty,
}

/// Discrete incident report categories from the low byte of IncidentFlags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentReport {
    /// No incident was reported (code `0x00`).
    NoReport,
    /// Driver lost control of the vehicle (code `0x01`).
    OutOfControl,
    /// Driver went off the racing surface (code `0x02`).
    OffTrack,
    /// Driver is continuing off the racing surface (code `0x03`).
    OffTrackOngoing,
    /// Driver made minor contact with a world object (code `0x04`).
    ContactWithWorld,
    /// Driver collided with a world object (code `0x05`).
    CollisionWithWorld,
    /// Driver is in an ongoing collision with a world object (code `0x06`).
    CollisionWithWorldOngoing,
    /// Driver made minor contact with another car (code `0x07`).
    ContactWithCar,
    /// Driver collided with another car (code `0x08`).
    CollisionWithCar,
    /// An unrecognised report code from the iRacing SDK.
    Unknown(i32),
}

impl IncidentReport {
    /// Constructs the variant corresponding to the given raw report code.
    ///
    /// Returns [`IncidentReport::Unknown`] for any code not listed above.
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

    /// Returns the raw report code for this variant.
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
    /// No penalty assigned (code `0x00`).
    None,
    /// 0x (zero-multiplier) penalty (code `0x01`).
    ZeroX,
    /// 1x penalty (code `0x02`).
    OneX,
    /// 2x penalty (code `0x03`).
    TwoX,
    /// 4x penalty (code `0x04`).
    FourX,
    /// An unrecognised penalty code from the iRacing SDK.
    Unknown(i32),
}

impl IncidentPenalty {
    /// Constructs the variant corresponding to the given raw penalty code.
    ///
    /// Returns [`IncidentPenalty::Unknown`] for any code not listed above.
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

    /// Returns the raw penalty code for this variant.
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
