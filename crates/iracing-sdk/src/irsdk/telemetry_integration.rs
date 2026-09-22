//! Integration between wire-contract values and telemetry schema decoding.

use iracing_irsdk::{
    BitField, BroadcastMessage, CameraState, CameraSwitchFocusMode, CarLeftRight, ChatCommandMode,
    EngineWarnings, ForceFeedbackCommandMode, IncidentFlags, PaceFlags, PaceMode, PitCommandMode,
    PitServiceFlags, PitServiceStatus, ReloadTexturesMode, ReplayPositionMode, ReplaySearchMode,
    ReplayStateMode, SessionFlags, SessionState, TelemetryCommandMode, TrackLocation, TrackSurface,
    TrackWetness, VideoCaptureMode,
};

use crate::{IRacingSDKError, VarData, irsdk::VariableType};

macro_rules! impl_enum_var_data {
    ($($type:ty),+ $(,)?) => {$ (
        impl VarData for $type {
            const ELEMENT_SIZE: usize = 4;

            fn accepts_type(data_type: VariableType) -> bool {
                data_type == VariableType::Integer
            }

            fn expected_type() -> &'static str {
                "Integer"
            }

            fn decode_value(bytes: &[u8], data_type: VariableType) -> crate::Result<Self> {
                let raw = <i32 as VarData>::decode_value(bytes, data_type)?;
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
            const ELEMENT_SIZE: usize = 4;

            fn accepts_type(data_type: VariableType) -> bool {
                data_type == VariableType::BitField
            }

            fn expected_type() -> &'static str {
                "BitField"
            }

            fn decode_value(bytes: &[u8], data_type: VariableType) -> crate::Result<Self> {
                <BitField as VarData>::decode_value(bytes, data_type).map(Self::from)
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
    const ELEMENT_SIZE: usize = 4;

    fn accepts_type(data_type: VariableType) -> bool {
        matches!(data_type, VariableType::BitField | VariableType::Integer)
    }

    fn expected_type() -> &'static str {
        "BitField or Int32"
    }

    fn decode_value(bytes: &[u8], data_type: VariableType) -> crate::Result<Self> {
        match data_type {
            VariableType::BitField => {
                <BitField as VarData>::decode_value(bytes, data_type).map(Self::from)
            }
            VariableType::Integer => <i32 as VarData>::decode_value(bytes, data_type)
                .map(|value| Self::from(value as u32)),
            actual => Err(IRacingSDKError::type_conversion(
                Self::expected_type(),
                actual,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VariableInfo;

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

    #[test]
    fn sdk_enum_and_flag_arrays_decode_from_checked_chunks() {
        let mut enum_info = variable_info(VariableType::Integer);
        enum_info.count = 2;
        let mut enum_bytes = Vec::new();
        enum_bytes.extend_from_slice(&i32::from(SessionState::Racing).to_le_bytes());
        enum_bytes.extend_from_slice(&i32::from(SessionState::Racing).to_le_bytes());
        assert_eq!(
            enum_info.decode::<Vec<SessionState>>(&enum_bytes).unwrap(),
            vec![SessionState::Racing; 2]
        );

        let mut flags_info = variable_info(VariableType::BitField);
        flags_info.count = 2;
        let flags_bytes = [
            SessionFlags::GREEN.bits().to_le_bytes(),
            SessionFlags::CHECKERED.bits().to_le_bytes(),
        ]
        .concat();
        assert_eq!(
            flags_info
                .decode::<Vec<SessionFlags>>(&flags_bytes)
                .unwrap(),
            vec![SessionFlags::GREEN, SessionFlags::CHECKERED]
        );
    }
}
