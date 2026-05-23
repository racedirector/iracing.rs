use tonic::Status;

use crate::broadcast::*;

use super::request;

use iracing_sdk::{BroadcastCommand, CameraState, PitCommand};

pub(crate) fn camera_state(state: u32) -> CameraState {
    CameraState::from_bits_retain(state)
}

pub(crate) fn replay_position_mode(mode: ReplayPositionMode) -> iracing_sdk::ReplayPositionMode {
    match mode {
        ReplayPositionMode::Begin => iracing_sdk::ReplayPositionMode::Begin,
        ReplayPositionMode::Current => iracing_sdk::ReplayPositionMode::Current,
        ReplayPositionMode::End => iracing_sdk::ReplayPositionMode::End,
        ReplayPositionMode::Unknown => unreachable!("unknown replay position mode is rejected"),
    }
}

pub(crate) fn replay_search_mode(mode: ReplaySearchMode) -> iracing_sdk::ReplaySearchMode {
    match mode {
        ReplaySearchMode::ToStart => iracing_sdk::ReplaySearchMode::ToStart,
        ReplaySearchMode::ToEnd => iracing_sdk::ReplaySearchMode::ToEnd,
        ReplaySearchMode::PreviousSession => iracing_sdk::ReplaySearchMode::PrevSession,
        ReplaySearchMode::NextSession => iracing_sdk::ReplaySearchMode::NextSession,
        ReplaySearchMode::PreviousLap => iracing_sdk::ReplaySearchMode::PrevLap,
        ReplaySearchMode::NextLap => iracing_sdk::ReplaySearchMode::NextLap,
        ReplaySearchMode::PreviousFrame => iracing_sdk::ReplaySearchMode::PrevFrame,
        ReplaySearchMode::NextFrame => iracing_sdk::ReplaySearchMode::NextFrame,
        ReplaySearchMode::PreviousIncident => iracing_sdk::ReplaySearchMode::PrevIncident,
        ReplaySearchMode::NextIncident => iracing_sdk::ReplaySearchMode::NextIncident,
        ReplaySearchMode::Unknown => unreachable!("unknown replay search mode is rejected"),
    }
}

pub(crate) fn replay_state_mode(mode: ReplayStateMode) -> iracing_sdk::ReplayStateMode {
    match mode {
        ReplayStateMode::EraseTape => iracing_sdk::ReplayStateMode::EraseTape,
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
        ChatCommandMode::BeginChat => {
            BroadcastCommand::ChatCommand(iracing_sdk::ChatCommandMode::BeginChat)
        }
        ChatCommandMode::Reply => {
            BroadcastCommand::ChatCommand(iracing_sdk::ChatCommandMode::Reply)
        }
        ChatCommandMode::Cancel => {
            BroadcastCommand::ChatCommand(iracing_sdk::ChatCommandMode::Cancel)
        }
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

pub(crate) fn telemetry_command_mode(
    mode: TelemetryCommandMode,
) -> iracing_sdk::TelemetryCommandMode {
    match mode {
        TelemetryCommandMode::Stop => iracing_sdk::TelemetryCommandMode::Stop,
        TelemetryCommandMode::Start => iracing_sdk::TelemetryCommandMode::Start,
        TelemetryCommandMode::Restart => iracing_sdk::TelemetryCommandMode::Restart,
        TelemetryCommandMode::Unknown => unreachable!("unknown telemetry command mode is rejected"),
    }
}

pub(crate) fn video_capture_mode(mode: VideoCaptureMode) -> iracing_sdk::VideoCaptureMode {
    match mode {
        VideoCaptureMode::Screenshot => iracing_sdk::VideoCaptureMode::TriggerScreenShot,
        VideoCaptureMode::Start => iracing_sdk::VideoCaptureMode::StartVideoCapture,
        VideoCaptureMode::Stop => iracing_sdk::VideoCaptureMode::EndVideoCapture,
        VideoCaptureMode::Toggle => iracing_sdk::VideoCaptureMode::ToggleVideoCapture,
        VideoCaptureMode::ShowTimer => iracing_sdk::VideoCaptureMode::ShowVideoTimer,
        VideoCaptureMode::HideTimer => iracing_sdk::VideoCaptureMode::HideVideoTimer,
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
            BroadcastCommand::ChatCommand(iracing_sdk::ChatCommandMode::Reply)
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

    #[test]
    fn pit_command_converts_all_no_value_modes() {
        let no_value_cases = [
            (PitCommandMode::TearOff, PitCommand::Tearoff),
            (PitCommandMode::ClearTires, PitCommand::ClearTires),
            (PitCommandMode::FastRepair, PitCommand::FastRepair),
            (PitCommandMode::ClearTearOff, PitCommand::ClearTearoff),
            (PitCommandMode::ClearFastRepair, PitCommand::ClearFastRepair),
            (PitCommandMode::ClearFuel, PitCommand::ClearFuel),
        ];
        for (mode, expected) in no_value_cases {
            assert_eq!(
                pit_command(PitCommandRequest {
                    mode: Some(mode as i32),
                    value: None,
                })
                .unwrap(),
                expected,
                "mode {:?} should convert correctly",
                mode
            );
        }
    }

    #[test]
    fn pit_command_converts_all_tire_pressure_modes() {
        let tire_cases = [
            (PitCommandMode::LfTire, PitCommand::LF(20)),
            (PitCommandMode::RfTire, PitCommand::RF(21)),
            (PitCommandMode::LrTire, PitCommand::LR(22)),
            (PitCommandMode::RrTire, PitCommand::RR(23)),
        ];
        for (mode, expected) in tire_cases {
            let value = match expected {
                PitCommand::LF(v) | PitCommand::RF(v) | PitCommand::LR(v) | PitCommand::RR(v) => {
                    f32::from(v)
                }
                _ => unreachable!(),
            };
            assert_eq!(
                pit_command(PitCommandRequest {
                    mode: Some(mode as i32),
                    value: Some(value),
                })
                .unwrap(),
                expected,
                "mode {:?} should convert correctly",
                mode
            );
        }
    }

    #[test]
    fn replay_position_mode_converts_all_variants() {
        assert_eq!(
            replay_position_mode(ReplayPositionMode::Begin),
            iracing_sdk::ReplayPositionMode::Begin
        );
        assert_eq!(
            replay_position_mode(ReplayPositionMode::Current),
            iracing_sdk::ReplayPositionMode::Current
        );
        assert_eq!(
            replay_position_mode(ReplayPositionMode::End),
            iracing_sdk::ReplayPositionMode::End
        );
    }

    #[test]
    fn replay_search_mode_converts_all_variants() {
        let cases = [
            (ReplaySearchMode::ToStart, iracing_sdk::ReplaySearchMode::ToStart),
            (ReplaySearchMode::ToEnd, iracing_sdk::ReplaySearchMode::ToEnd),
            (ReplaySearchMode::PreviousSession, iracing_sdk::ReplaySearchMode::PrevSession),
            (ReplaySearchMode::NextSession, iracing_sdk::ReplaySearchMode::NextSession),
            (ReplaySearchMode::PreviousLap, iracing_sdk::ReplaySearchMode::PrevLap),
            (ReplaySearchMode::NextLap, iracing_sdk::ReplaySearchMode::NextLap),
            (ReplaySearchMode::PreviousFrame, iracing_sdk::ReplaySearchMode::PrevFrame),
            (ReplaySearchMode::NextFrame, iracing_sdk::ReplaySearchMode::NextFrame),
            (ReplaySearchMode::PreviousIncident, iracing_sdk::ReplaySearchMode::PrevIncident),
            (ReplaySearchMode::NextIncident, iracing_sdk::ReplaySearchMode::NextIncident),
        ];
        for (proto_mode, expected) in cases {
            assert_eq!(
                replay_search_mode(proto_mode),
                expected,
                "replay search mode {:?} should convert",
                proto_mode
            );
        }
    }

    #[test]
    fn replay_state_mode_converts_erase_tape() {
        assert_eq!(
            replay_state_mode(ReplayStateMode::EraseTape),
            iracing_sdk::ReplayStateMode::EraseTape
        );
    }

    #[test]
    fn telemetry_command_mode_converts_all_variants() {
        assert_eq!(
            telemetry_command_mode(TelemetryCommandMode::Stop),
            iracing_sdk::TelemetryCommandMode::Stop
        );
        assert_eq!(
            telemetry_command_mode(TelemetryCommandMode::Start),
            iracing_sdk::TelemetryCommandMode::Start
        );
        assert_eq!(
            telemetry_command_mode(TelemetryCommandMode::Restart),
            iracing_sdk::TelemetryCommandMode::Restart
        );
    }

    #[test]
    fn video_capture_mode_converts_all_variants() {
        let cases = [
            (VideoCaptureMode::Screenshot, iracing_sdk::VideoCaptureMode::TriggerScreenShot),
            (VideoCaptureMode::Start, iracing_sdk::VideoCaptureMode::StartVideoCapture),
            (VideoCaptureMode::Stop, iracing_sdk::VideoCaptureMode::EndVideoCapture),
            (VideoCaptureMode::Toggle, iracing_sdk::VideoCaptureMode::ToggleVideoCapture),
            (VideoCaptureMode::ShowTimer, iracing_sdk::VideoCaptureMode::ShowVideoTimer),
            (VideoCaptureMode::HideTimer, iracing_sdk::VideoCaptureMode::HideVideoTimer),
        ];
        for (proto_mode, expected) in cases {
            assert_eq!(
                video_capture_mode(proto_mode),
                expected,
                "video capture mode {:?} should convert",
                proto_mode
            );
        }
    }

    #[test]
    fn camera_state_converts_bits() {
        use iracing_sdk::CameraState;

        let state_val = CameraState::UI_HIDDEN | CameraState::IS_SESSION_SCREEN;
        let result = camera_state(state_val.bits());
        assert_eq!(result, state_val);

        // Zero bits should round-trip
        let empty = camera_state(0);
        assert_eq!(empty.bits(), 0);
    }
}
