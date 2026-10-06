use crate::{command::Command, error::BroadcastError, pad_car_number::pad_car_number};
use iracing_irsdk::{
    BroadcastMessage, ChatCommandMode, ForceFeedbackCommandMode, PitCommand, PitCommandMode,
    ReloadTexturesMode,
};

pub type FormattedMessage = (BroadcastMessage, u16, u16, u16);

fn split_u32_words(value: u32) -> (u16, u16) {
    ((value & 0xFFFF) as u16, ((value >> 16) & 0xFFFF) as u16)
}

fn encode_mode<T: Into<i32>>(mode: T) -> u16 {
    u16::try_from(mode.into()).expect("broadcast modes must fit in u16")
}

impl TryFrom<Command> for FormattedMessage {
    type Error = BroadcastError;

    fn try_from(command: Command) -> std::result::Result<Self, Self::Error> {
        let message = match command {
            Command::CameraSwitchPosition(position, group, camera) => (
                BroadcastMessage::CameraSwitchPosition,
                position,
                group,
                camera,
            ),
            Command::CameraSwitchNumber(car_number, group, camera) => (
                BroadcastMessage::CameraSwitchNumber,
                pad_car_number(&car_number),
                group,
                camera,
            ),
            Command::CameraSetState(camera_state) => (
                BroadcastMessage::CameraSetState,
                camera_state.bits() as u16,
                0,
                0,
            ),
            Command::ReplaySetPlaySpeed(speed, slow_motion) => (
                BroadcastMessage::ReplaySetPlaySpeed,
                speed as u16,
                slow_motion.into(),
                0,
            ),
            Command::ReplaySetPlayPosition(mode, frame_number) => {
                let (low, high) = split_u32_words(frame_number);
                (
                    BroadcastMessage::ReplaySetPlayPosition,
                    encode_mode(mode),
                    low,
                    high,
                )
            }
            Command::ReplaySearch(mode) => {
                (BroadcastMessage::ReplaySearch, encode_mode(mode), 0, 0)
            }
            Command::ReplaySetState(mode) => {
                (BroadcastMessage::ReplaySetState, encode_mode(mode), 0, 0)
            }
            Command::ReloadAllTextures => (
                BroadcastMessage::ReloadTextures,
                encode_mode(ReloadTexturesMode::All),
                0,
                0,
            ),
            Command::ReloadTextures(car_index) => (
                BroadcastMessage::ReloadTextures,
                encode_mode(ReloadTexturesMode::CarIndex),
                car_index,
                0,
            ),
            Command::ChatCommand(mode) => (BroadcastMessage::ChatCommand, encode_mode(mode), 0, 0),
            Command::ChatCommandMacro(macro_number) => {
                if !(1..=15).contains(&macro_number) {
                    return Err(BroadcastError::Validation {
                        reason: format!("macro id must be in range 1..=15, got {macro_number}"),
                    });
                }

                (
                    BroadcastMessage::ChatCommand,
                    encode_mode(ChatCommandMode::Macro),
                    macro_number,
                    0,
                )
            }
            Command::PitCommand(pit_command_mode) => {
                let (var1, var2) = match pit_command_mode {
                    PitCommand::Clear => (encode_mode(PitCommandMode::Clear), 0),
                    PitCommand::Tearoff => (encode_mode(PitCommandMode::WindshieldTearoff), 0),
                    PitCommand::Fuel(gal) => (encode_mode(PitCommandMode::Fuel), gal),
                    PitCommand::LF(pressure) => {
                        (encode_mode(PitCommandMode::LeftFrontTire), pressure)
                    }
                    PitCommand::RF(pressure) => {
                        (encode_mode(PitCommandMode::RightFrontTire), pressure)
                    }
                    PitCommand::LR(pressure) => {
                        (encode_mode(PitCommandMode::LeftRearTire), pressure)
                    }
                    PitCommand::RR(pressure) => {
                        (encode_mode(PitCommandMode::RightRearTire), pressure)
                    }
                    PitCommand::ClearTires => (encode_mode(PitCommandMode::ClearTires), 0),
                    PitCommand::FastRepair => (encode_mode(PitCommandMode::FastRepair), 0),
                    PitCommand::ClearTearoff => {
                        (encode_mode(PitCommandMode::ClearWindshieldTearoff), 0)
                    }
                    PitCommand::ClearFastRepair => {
                        (encode_mode(PitCommandMode::ClearFastRepair), 0)
                    }
                    PitCommand::ClearFuel => (encode_mode(PitCommandMode::ClearFuel), 0),
                };

                (BroadcastMessage::PitCommand, var1, var2, 0)
            }
            Command::TelemetryCommand(mode) => {
                (BroadcastMessage::TelemetryCommand, encode_mode(mode), 0, 0)
            }
            Command::FFBCommand(value) => {
                let bits = value.to_bits();
                let (low, high) = split_u32_words(bits);
                (
                    BroadcastMessage::ForceFeedbackCommand,
                    encode_mode(ForceFeedbackCommandMode::MaxForce),
                    low,
                    high,
                )
            }
            Command::ReplaySearchSessionTime(session_number, session_time_ms) => {
                let (low, high) = split_u32_words(session_time_ms);
                (
                    BroadcastMessage::ReplaySearchSessionTime,
                    session_number,
                    low,
                    high,
                )
            }
            Command::VideoCapture(mode) => {
                (BroadcastMessage::VideoCapture, encode_mode(mode), 0, 0)
            }
        };

        Ok(message)
    }
}
