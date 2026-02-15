//! Incident classification types for IRSDK 1.19

use serde::{Deserialize, Serialize};

use super::BitField;

/// High-level classification of an incident: report + penalty
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentClassification {
    pub report: IncidentReport,
    pub penalty: IncidentPenalty,
}

/// Discrete incident report categories from the low byte
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    Unknown(u8),
}

/// Discrete incident penalty magnitudes from the high byte
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentPenalty {
    None,
    ZeroX,
    OneX,
    TwoX,
    FourX,
    Unknown(u8),
}

/// Decode a BitField carrying IRSDK 1.19 IncidentFlags into a structured classification
pub fn decode_incident(bits: BitField) -> IncidentClassification {
    use super::irsdk_flags::incident as inc;

    let raw = bits.value();
    let rep = (raw & inc::REP_MASK) as u8;
    let pen = ((raw & inc::PEN_MASK) >> 8) as u8;
    let report = match rep {
        inc::REP_NO_REPORT => IncidentReport::NoReport,
        inc::REP_OUT_OF_CONTROL => IncidentReport::OutOfControl,
        inc::REP_OFF_TRACK => IncidentReport::OffTrack,
        inc::REP_OFF_TRACK_ONGOING => IncidentReport::OffTrackOngoing,
        inc::REP_CONTACT_WITH_WORLD => IncidentReport::ContactWithWorld,
        inc::REP_COLLISION_WITH_WORLD => IncidentReport::CollisionWithWorld,
        inc::REP_COLLISION_WITH_WORLD_ONGOING => IncidentReport::CollisionWithWorldOngoing,
        inc::REP_CONTACT_WITH_CAR => IncidentReport::ContactWithCar,
        inc::REP_COLLISION_WITH_CAR => IncidentReport::CollisionWithCar,
        other => IncidentReport::Unknown(other),
    };

    let penalty = match pen {
        inc::PEN_NONE => IncidentPenalty::None,
        inc::PEN_0X => IncidentPenalty::ZeroX,
        inc::PEN_1X => IncidentPenalty::OneX,
        inc::PEN_2X => IncidentPenalty::TwoX,
        inc::PEN_4X => IncidentPenalty::FourX,
        other => IncidentPenalty::Unknown(other),
    };

    IncidentClassification { report, penalty }
}
