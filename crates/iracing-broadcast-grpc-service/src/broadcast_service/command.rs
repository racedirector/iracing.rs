use tonic::Status;

use crate::broadcast::*;

use super::request;

use iracing_sdk::types::irsdk::{
    CameraState, ChatCommandMode as SdkChatCommandMode,
    ReplayPositionMode as SdkReplayPositionMode, ReplaySearchMode as SdkReplaySearchMode,
    ReplayStateMode as SdkReplayStateMode, TelemetryCommandMode as SdkTelemetryCommandMode,
    VideoCaptureMode as SdkVideoCaptureMode,
};
use iracing_sdk::{BroadcastCommand, PitCommand};

pub(crate) fn camera_state(state: u32) -> CameraState {
    CameraState::from_bits_retain(state)
}

pub(crate) fn replay_position_mode(mode: ReplayPositionMode) -> SdkReplayPositionMode {
    match mode {
        ReplayPositionMode::Begin => SdkReplayPositionMode::Begin,
        ReplayPositionMode::Current => SdkReplayPositionMode::Current,
        ReplayPositionMode::End => SdkReplayPositionMode::End,
        ReplayPositionMode::Unknown => unreachable!("unknown replay position mode is rejected"),
    }
}

pub(crate) fn replay_search_mode(mode: ReplaySearchMode) -> SdkReplaySearchMode {
    match mode {
        ReplaySearchMode::ToStart => SdkReplaySearchMode::ToStart,
        ReplaySearchMode::ToEnd => SdkReplaySearchMode::ToEnd,
        ReplaySearchMode::PreviousSession => SdkReplaySearchMode::PrevSession,
        ReplaySearchMode::NextSession => SdkReplaySearchMode::NextSession,
        ReplaySearchMode::PreviousLap => SdkReplaySearchMode::PrevLap,
        ReplaySearchMode::NextLap => SdkReplaySearchMode::NextLap,
        ReplaySearchMode::PreviousFrame => SdkReplaySearchMode::PrevFrame,
        ReplaySearchMode::NextFrame => SdkReplaySearchMode::NextFrame,
        ReplaySearchMode::PreviousIncident => SdkReplaySearchMode::PrevIncident,
        ReplaySearchMode::NextIncident => SdkReplaySearchMode::NextIncident,
        ReplaySearchMode::Unknown => unreachable!("unknown replay search mode is rejected"),
    }
}

pub(crate) fn replay_state_mode(mode: ReplayStateMode) -> SdkReplayStateMode {
    match mode {
        ReplayStateMode::EraseTape => SdkReplayStateMode::EraseTape,
        ReplayStateMode::Unknown => unreachable!("unknown replay state mode is rejected"),
    }
}

pub(crate) fn chat_command(request: ChatCommandRequest) -> Result<BroadcastCommand, Status> {
    let ChatCommandRequest { mode, r#macro } = request;
    let mode = request::required_enum::<ChatCommandMode>("mode", mode)?;

    Ok(match mode {
        ChatCommandMode::Macro => {
            let macro_number = required_chat_macro(r#macro)?;
            BroadcastCommand::ChatCommandMacro(macro_number)
        }
        ChatCommandMode::BeginChat => BroadcastCommand::ChatCommand(SdkChatCommandMode::BeginChat),
        ChatCommandMode::Reply => BroadcastCommand::ChatCommand(SdkChatCommandMode::Reply),
        ChatCommandMode::Cancel => BroadcastCommand::ChatCommand(SdkChatCommandMode::Cancel),
        ChatCommandMode::Unknown => unreachable!("unknown chat command mode is rejected"),
    })
}

pub(crate) fn pit_command(request: PitCommandRequest) -> Result<PitCommand, Status> {
    let PitCommandRequest { mode, value } = request;
    let mode = request::required_enum::<PitCommandMode>("mode", mode)?;

    Ok(match mode {
        PitCommandMode::Clear => PitCommand::Clear,
        PitCommandMode::TearOff => PitCommand::Tearoff,
        PitCommandMode::Fuel => {
            let value = request::required_f32("value", value)?;
            PitCommand::Fuel(request::f32_to_u16("value", value)?)
        }
        PitCommandMode::LfTire => {
            let value = request::required_f32("value", value)?;
            PitCommand::LF(request::f32_to_u16("value", value)?)
        }
        PitCommandMode::RfTire => {
            let value = request::required_f32("value", value)?;
            PitCommand::RF(request::f32_to_u16("value", value)?)
        }
        PitCommandMode::LrTire => {
            let value = request::required_f32("value", value)?;
            PitCommand::LR(request::f32_to_u16("value", value)?)
        }
        PitCommandMode::RrTire => {
            let value = request::required_f32("value", value)?;
            PitCommand::RR(request::f32_to_u16("value", value)?)
        }
        PitCommandMode::ClearTires => PitCommand::ClearTires,
        PitCommandMode::FastRepair => PitCommand::FastRepair,
        PitCommandMode::ClearTearOff => PitCommand::ClearTearoff,
        PitCommandMode::ClearFastRepair => PitCommand::ClearFastRepair,
        PitCommandMode::ClearFuel => PitCommand::ClearFuel,
        PitCommandMode::Unknown => unreachable!("unknown pit command mode is rejected"),
    })
}

pub(crate) fn telemetry_command_mode(mode: TelemetryCommandMode) -> SdkTelemetryCommandMode {
    match mode {
        TelemetryCommandMode::Stop => SdkTelemetryCommandMode::Stop,
        TelemetryCommandMode::Start => SdkTelemetryCommandMode::Start,
        TelemetryCommandMode::Restart => SdkTelemetryCommandMode::Restart,
        TelemetryCommandMode::Unknown => unreachable!("unknown telemetry command mode is rejected"),
    }
}

pub(crate) fn video_capture_mode(mode: VideoCaptureMode) -> SdkVideoCaptureMode {
    match mode {
        VideoCaptureMode::Screenshot => SdkVideoCaptureMode::TriggerScreenshot,
        VideoCaptureMode::Start => SdkVideoCaptureMode::StartVideoCapture,
        VideoCaptureMode::Stop => SdkVideoCaptureMode::EndVideoCapture,
        VideoCaptureMode::Toggle => SdkVideoCaptureMode::ToggleVideoCapture,
        VideoCaptureMode::ShowTimer => SdkVideoCaptureMode::ShowVideoTimer,
        VideoCaptureMode::HideTimer => SdkVideoCaptureMode::HideVideoTimer,
        VideoCaptureMode::Unknown => unreachable!("unknown video capture mode is rejected"),
    }
}

fn required_chat_macro(value: Option<u32>) -> Result<u16, Status> {
    let macro_number = request::required_u16("macro", value)?;

    if !(1..=15).contains(&macro_number) {
        return Err(Status::invalid_argument(format!(
            "`macro` must be in the range 1..=15, got {macro_number}"
        )));
    }

    Ok(macro_number)
}

#[cfg(test)]
mod tests {
    use tonic::Code;

    use super::*;

    fn assert_invalid_argument(error: Status, field: &str) {
        assert_eq!(error.code(), Code::InvalidArgument);
        assert!(
            error.message().contains(field),
            "error message should mention `{field}`: {}",
            error.message()
        );
    }

    #[test]
    fn chat_command_converts_macro_and_standard_modes() {
        assert_eq!(
            chat_command(ChatCommandRequest {
                mode: Some(ChatCommandMode::Macro as i32),
                r#macro: Some(3),
            })
            .unwrap(),
            BroadcastCommand::ChatCommandMacro(3)
        );
        assert_eq!(
            chat_command(ChatCommandRequest {
                mode: Some(ChatCommandMode::Reply as i32),
                r#macro: None,
            })
            .unwrap(),
            BroadcastCommand::ChatCommand(SdkChatCommandMode::Reply)
        );
    }

    #[test]
    fn chat_command_rejects_missing_unknown_and_invalid_macro_values() {
        assert_invalid_argument(
            chat_command(ChatCommandRequest {
                mode: None,
                r#macro: None,
            })
            .unwrap_err(),
            "mode",
        );
        assert_invalid_argument(
            chat_command(ChatCommandRequest {
                mode: Some(ChatCommandMode::Unknown as i32),
                r#macro: None,
            })
            .unwrap_err(),
            "mode",
        );
        assert_invalid_argument(
            chat_command(ChatCommandRequest {
                mode: Some(ChatCommandMode::Macro as i32),
                r#macro: None,
            })
            .unwrap_err(),
            "macro",
        );
        assert_invalid_argument(
            chat_command(ChatCommandRequest {
                mode: Some(ChatCommandMode::Macro as i32),
                r#macro: Some(0),
            })
            .unwrap_err(),
            "macro",
        );
        assert_invalid_argument(
            chat_command(ChatCommandRequest {
                mode: Some(ChatCommandMode::Macro as i32),
                r#macro: Some(16),
            })
            .unwrap_err(),
            "macro",
        );
    }

    #[test]
    fn pit_command_converts_clear_and_value_modes() {
        assert_eq!(
            pit_command(PitCommandRequest {
                mode: Some(PitCommandMode::Clear as i32),
                value: None,
            })
            .unwrap(),
            PitCommand::Clear
        );
        assert_eq!(
            pit_command(PitCommandRequest {
                mode: Some(PitCommandMode::Fuel as i32),
                value: Some(5.0),
            })
            .unwrap(),
            PitCommand::Fuel(5)
        );
        assert_eq!(
            pit_command(PitCommandRequest {
                mode: Some(PitCommandMode::RfTire as i32),
                value: Some(21.0),
            })
            .unwrap(),
            PitCommand::RF(21)
        );
    }

    #[test]
    fn pit_command_rejects_missing_unknown_and_invalid_values() {
        assert_invalid_argument(
            pit_command(PitCommandRequest {
                mode: None,
                value: None,
            })
            .unwrap_err(),
            "mode",
        );
        assert_invalid_argument(
            pit_command(PitCommandRequest {
                mode: Some(PitCommandMode::Unknown as i32),
                value: None,
            })
            .unwrap_err(),
            "mode",
        );
        assert_invalid_argument(
            pit_command(PitCommandRequest {
                mode: Some(PitCommandMode::Fuel as i32),
                value: None,
            })
            .unwrap_err(),
            "value",
        );
        assert_invalid_argument(
            pit_command(PitCommandRequest {
                mode: Some(PitCommandMode::Fuel as i32),
                value: Some(1.5),
            })
            .unwrap_err(),
            "value",
        );
        assert_invalid_argument(
            pit_command(PitCommandRequest {
                mode: Some(PitCommandMode::Fuel as i32),
                value: Some(f32::INFINITY),
            })
            .unwrap_err(),
            "value",
        );
        assert_invalid_argument(
            pit_command(PitCommandRequest {
                mode: Some(PitCommandMode::Fuel as i32),
                value: Some(f32::from(u16::MAX) + 1.0),
            })
            .unwrap_err(),
            "value",
        );
    }
}
