//! Integration between wire-contract values and telemetry schema decoding.

use iracing_irsdk::{
    BitField, BroadcastMessage, CameraState, CameraSwitchFocusMode, CarLeftRight, ChatCommandMode,
    EngineWarnings, ForceFeedbackCommandMode, IncidentFlags, PaceFlags, PaceMode, PitCommandMode,
    PitServiceFlags, PitServiceStatus, ReloadTexturesMode, ReplayPositionMode, ReplaySearchMode,
    ReplayStateMode, SessionFlags, SessionState, TelemetryCommandMode, TrackLocation, TrackSurface,
    TrackWetness, VideoCaptureMode,
};

use crate::{IRacingSDKError, VarData, VariableInfo};

macro_rules! impl_enum_var_data {
    ($($type:ty),+ $(,)?) => {$ (
        impl VarData for $type {
            fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
                let raw = <i32 as VarData>::from_bytes(data, info)?;
                Self::try_from(raw).map_err(|raw| {
                    IRacingSDKError::parse_error(
                        concat!("unknown ", stringify!($type), " value"),
                        raw.to_string(),
                    )
                })
            }
        }
    )+};
}

impl_enum_var_data!(
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
    VideoCaptureMode,
);

macro_rules! impl_bitmask_var_data {
    ($($type:ty),+ $(,)?) => {$ (
        impl VarData for $type {
            fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
                if info.data_type != iracing_irsdk::VariableType::BitField {
                    return Err(IRacingSDKError::type_conversion("BitField", info.data_type));
                }

                <BitField as VarData>::from_bytes(data, info).map(Self::from)
            }
        }
    )+};
}

impl_bitmask_var_data!(
    CameraState,
    EngineWarnings,
    PaceFlags,
    PitServiceFlags,
    SessionFlags,
);

impl VarData for IncidentFlags {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        match info.data_type {
            iracing_irsdk::VariableType::BitField => {
                <BitField as VarData>::from_bytes(data, info).map(Self::from)
            }
            iracing_irsdk::VariableType::Integer => {
                <i32 as VarData>::from_bytes(data, info).map(|value| Self::from(value as u32))
            }
            actual => Err(IRacingSDKError::type_conversion(
                "BitField or Int32",
                actual,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iracing_irsdk::VariableType;

    fn variable_info(data_type: VariableType) -> VariableInfo {
        VariableInfo {
            name: "test".to_owned(),
            data_type,
            offset: 0,
            count: 1,
            count_as_time: false,
            units: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn enum_and_bitmask_wire_types_still_decode_through_var_data() {
        assert_eq!(
            SessionState::from_bytes(
                &i32::from(SessionState::Racing).to_le_bytes(),
                &variable_info(VariableType::Integer),
            )
            .unwrap(),
            SessionState::Racing
        );
        assert_eq!(
            SessionFlags::from_bytes(
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
                IncidentFlags::from_bytes(&RAW.to_le_bytes(), &variable_info(data_type)).unwrap();
            assert_eq!(decoded.bits(), RAW);
        }
    }
}
