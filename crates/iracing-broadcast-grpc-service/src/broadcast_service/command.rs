use tonic::Status;

use crate::broadcast::*;

use super::request;

use iracing_sdk::{BroadcastCommand, PitCommand};

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
