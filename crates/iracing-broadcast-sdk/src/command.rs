use iracing_irsdk::{
    BroadcastMessage, BroadcastMessageKind, CameraState, ChatCommandMode, ForceFeedbackCommandMode,
    PitCommand, PitCommandMode, ReloadTexturesMode, ReplayPositionMode, ReplaySearchMode,
    ReplayStateMode, TelemetryCommandMode, VideoCaptureMode,
};

use crate::pad_car_number::pad_car_number;

/// Messages that can be sent to the iRacing simulation.
///
/// Each variant maps to the documented window message contract in the iRacing
/// SDK. Primitive parameters are passed through as-is and packed into the
/// `WPARAM`/`LPARAM` pairs expected by the simulator.
///
/// # Examples
///
/// ```
/// use iracing_broadcast_sdk::{Command, PitCommand};
///
/// let _ = Command::CameraSwitchPosition(0, 0, 0);
/// let _ = Command::Pit(PitCommand::Fuel(8));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Switch to a specific camera group and camera index for a position.
    CameraSwitchPosition(u16, u16, u16),
    /// Switch to a specific camera group and camera index for a car number.
    CameraSwitchNumber(String, u16, u16),
    /// Apply a new [`CameraState`] bitfield.
    CameraSetState(CameraState),
    /// Set the replay play speed, with an optional slow-motion toggle.
    ReplaySetPlaySpeed(i16, bool),
    /// Jump to a replay position, with the frame number split across `var2`/`var3`.
    ReplaySetPlayPosition(ReplayPositionMode, u32),
    /// Perform a replay search according to the provided mode.
    ReplaySearch(ReplaySearchMode),
    /// Toggle the replay state on or off.
    ReplaySetState(ReplayStateMode),
    /// Reload all textures.
    ReloadAllTextures,
    /// Reload textures for a specific car index.
    ReloadTextures(u16),
    /// Send a chat command.
    Chat(ChatCommandMode),
    /// Send a chat macro by number.
    ChatMacro(u16),
    /// Issue a pit command.
    Pit(PitCommand),
    /// Control telemetry recording.
    Telemetry(TelemetryCommandMode),
    /// Send a force-feedback command.
    ForceFeedback(f32),
    /// Search a replay to a specific session time.
    ReplaySearchSessionTime(u16, u32),
    /// Control video capture.
    VideoCapture(VideoCaptureMode),
}

fn split_u32_words(value: u32) -> (u16, u16) {
    ((value & 0xffff) as u16, ((value >> 16) & 0xffff) as u16)
}

fn encode_mode<T: Into<i32>>(mode: T) -> u16 {
    u16::try_from(mode.into()).expect("broadcast modes must fit in u16")
}

macro_rules! broadcast_message {
    ($kind:expr) => {
        BroadcastMessage::new($kind, 0, 0, 0)
    };
    ($kind:expr, $v1:expr) => {
        BroadcastMessage::new($kind, $v1, 0, 0)
    };
    ($kind:expr, $v1:expr, $v2:expr) => {
        BroadcastMessage::new($kind, $v1, $v2, 0)
    };
    ($kind:expr, $v1:expr, $v2:expr, $v3:expr) => {
        BroadcastMessage::new($kind, $v1, $v2, $v3)
    };
}

impl TryFrom<Command> for BroadcastMessage {
    type Error = crate::error::Error;

    fn try_from(value: Command) -> Result<Self, Self::Error> {
        let message = match value {
            Command::CameraSwitchPosition(position, group, camera) => broadcast_message!(
                BroadcastMessageKind::CameraSwitchPosition,
                position,
                group,
                camera
            ),

            Command::CameraSwitchNumber(car_number, group, camera) => broadcast_message!(
                BroadcastMessageKind::CameraSwitchNumber,
                pad_car_number(&car_number),
                group,
                camera
            ),

            Command::CameraSetState(camera_state) => broadcast_message!(
                BroadcastMessageKind::CameraSetState,
                camera_state.bits() as u16
            ),

            Command::ReplaySetPlaySpeed(speed, slow_motion) => broadcast_message!(
                BroadcastMessageKind::ReplaySetPlaySpeed,
                speed as u16,
                slow_motion.into()
            ),

            Command::ReplaySetPlayPosition(mode, frame_number) => {
                let (low, high) = split_u32_words(frame_number);

                broadcast_message!(
                    BroadcastMessageKind::ReplaySetPlayPosition,
                    encode_mode(mode),
                    low,
                    high
                )
            }

            Command::ReplaySearch(mode) => {
                broadcast_message!(BroadcastMessageKind::ReplaySearch, encode_mode(mode))
            }

            Command::ReplaySetState(mode) => {
                broadcast_message!(BroadcastMessageKind::ReplaySetState, encode_mode(mode))
            }

            Command::ReloadAllTextures => broadcast_message!(
                BroadcastMessageKind::ReloadTextures,
                encode_mode(ReloadTexturesMode::All)
            ),

            Command::ReloadTextures(car_index) => broadcast_message!(
                BroadcastMessageKind::ReloadTextures,
                encode_mode(ReloadTexturesMode::CarIndex),
                car_index
            ),

            Command::Chat(mode) => {
                broadcast_message!(BroadcastMessageKind::ChatCommand, encode_mode(mode))
            }

            Command::ChatMacro(macro_number) => {
                if !(1..=15).contains(&macro_number) {
                    return Err(crate::error::Error::Validation {
                        reason: format!("macro id must be in range 1..=15, got {macro_number}"),
                    });
                }

                broadcast_message!(
                    BroadcastMessageKind::ChatCommand,
                    encode_mode(ChatCommandMode::Macro),
                    macro_number
                )
            }

            Command::Pit(pit_command) => {
                let (var1, var2) = match pit_command {
                    PitCommand::Clear => (encode_mode(PitCommandMode::Clear), 0),
                    PitCommand::Tearoff => (encode_mode(PitCommandMode::WindshieldTearoff), 0),
                    PitCommand::Fuel(gallons) => (encode_mode(PitCommandMode::Fuel), gallons),
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

                broadcast_message!(BroadcastMessageKind::PitCommand, var1, var2, 0)
            }

            Command::Telemetry(mode) => {
                broadcast_message!(BroadcastMessageKind::TelemetryCommand, encode_mode(mode))
            }

            Command::ForceFeedback(value) => {
                let (low, high) = split_u32_words(value.to_bits());

                broadcast_message!(
                    BroadcastMessageKind::ForceFeedbackCommand,
                    encode_mode(ForceFeedbackCommandMode::MaxForce),
                    low,
                    high
                )
            }

            Command::ReplaySearchSessionTime(session_number, session_time_ms) => {
                let (low, high) = split_u32_words(session_time_ms);

                broadcast_message!(
                    BroadcastMessageKind::ReplaySearchSessionTime,
                    session_number,
                    low,
                    high
                )
            }

            Command::VideoCapture(mode) => {
                broadcast_message!(BroadcastMessageKind::VideoCapture, encode_mode(mode))
            }
        };

        Ok(message)
    }
}
