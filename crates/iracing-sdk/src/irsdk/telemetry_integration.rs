//! Integration between wire-contract values and telemetry layout decoding.

use iracing_irsdk::{
    BitField, BroadcastMessage, CameraState, CameraSwitchFocusMode, CarLeftRight, ChatCommandMode,
    EngineWarnings, ForceFeedbackCommandMode, IncidentFlags, PaceFlags, PaceMode, PitCommandMode,
    PitServiceFlags, PitServiceStatus, ReloadTexturesMode, ReplayPositionMode, ReplaySearchMode,
    ReplayStateMode, SessionFlags, SessionState, TelemetryCommandMode, TrackLocation, TrackSurface,
    TrackWetness, VideoCaptureMode,
};

use crate::{FieldLayout, IRacingSDKError, TelemetryElement, VarData};
use iracing_irsdk::VariableType;
macro_rules! scalar_domain {
    ($type:ty, $raw:ty, $convert:expr) => {
        impl TelemetryElement for $type {
            const BYTE_WIDTH: usize = <$raw as TelemetryElement>::BYTE_WIDTH;
            fn accepts(storage: VariableType) -> bool {
                <$raw as TelemetryElement>::accepts(storage)
            }
            #[inline]
            fn decode_element(bytes: &[u8]) -> crate::Result<Self> {
                ($convert)(<$raw as TelemetryElement>::decode_element(bytes)?)
            }
        }
        impl VarData for $type {
            #[inline]
            fn validate_field(field: &FieldLayout) -> crate::Result<()> {
                <$raw as VarData>::validate_field(field)
            }
            #[inline]
            fn decode_prevalidated(frame: &[u8], field: &FieldLayout) -> crate::Result<Self> {
                Self::decode_element(crate::types::field_data::field_bytes(frame, field)?)
            }
        }
    };
}
macro_rules! enums {
    ($($type:ty),+ $(,)?) => {$ (
        scalar_domain!($type,i32,|raw| Self::try_from(raw).map_err(|raw:i32| IRacingSDKError::parse_error(concat!("unknown ", stringify!($type), " value"),raw.to_string())));
    )+};
}
enums!(
    BroadcastMessage,
    CameraSwitchFocusMode,
    CarLeftRight,
    ChatCommandMode,
    ForceFeedbackCommandMode,
    PaceMode,
    PitCommandMode,
    PitServiceStatus,
    ReloadTexturesMode,
    ReplayPositionMode,
    ReplaySearchMode,
    ReplayStateMode,
    SessionState,
    TelemetryCommandMode,
    TrackLocation,
    TrackSurface,
    TrackWetness,
    VideoCaptureMode
);
macro_rules! masks {
    ($($type:ty),+ $(,)?) => {$ (scalar_domain!($type,BitField,|raw| Ok(Self::from(raw)));)+};
}
masks!(
    CameraState,
    EngineWarnings,
    PaceFlags,
    PitServiceFlags,
    SessionFlags
);
impl TelemetryElement for IncidentFlags {
    const BYTE_WIDTH: usize = 4;
    fn accepts(storage: VariableType) -> bool {
        matches!(storage, VariableType::Integer | VariableType::BitField)
    }
    #[inline]
    fn decode_element(bytes: &[u8]) -> crate::Result<Self> {
        BitField::decode_element(bytes).map(Self::from)
    }
}
impl VarData for IncidentFlags {
    #[inline]
    fn validate_field(field: &FieldLayout) -> crate::Result<()> {
        if Self::accepts(field.data_type()) && field.count() == 1 {
            Ok(())
        } else {
            Err(IRacingSDKError::type_conversion(
                "scalar BitField or Integer",
                field.data_type(),
            ))
        }
    }
    #[inline]
    fn decode_prevalidated(frame: &[u8], field: &FieldLayout) -> crate::Result<Self> {
        Self::decode_element(crate::types::field_data::field_bytes(frame, field)?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iracing_irsdk::VariableType;

    fn variable_info(data_type: VariableType) -> FieldLayout {
        crate::test_utils::field(
            "test".to_owned(),
            data_type,
            0,
            1,
            false,
            String::new(),
            String::new(),
        )
    }

    #[test]
    fn enum_and_bitmask_wire_types_still_decode_through_var_data() {
        assert_eq!(
            SessionState::decode_field(
                &i32::from(SessionState::Racing).to_le_bytes(),
                &variable_info(VariableType::Integer),
            )
            .unwrap(),
            SessionState::Racing
        );
        assert_eq!(
            SessionFlags::decode_field(
                &SessionFlags::GREEN.bits().to_le_bytes(),
                &variable_info(VariableType::BitField),
            )
            .unwrap(),
            SessionFlags::GREEN
        );
    }

    #[test]
    fn incident_flags_accept_bitfield_and_integer_storage() {
        const RAW: u32 = 0x8000_0408;
        for data_type in [VariableType::BitField, VariableType::Integer] {
            let decoded =
                IncidentFlags::decode_field(&RAW.to_le_bytes(), &variable_info(data_type)).unwrap();
            assert_eq!(decoded.bits(), RAW);
        }
    }
}
