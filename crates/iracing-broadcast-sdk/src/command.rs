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

/// Returns the low and high 16-bit words, in that order, for broadcast arguments.
fn split_u32_words(value: u32) -> (u16, u16) {
    ((value & 0xffff) as u16, ((value >> 16) & 0xffff) as u16)
}

/// Encodes a broadcast mode as a protocol argument word.
///
/// # Panics
///
/// Panics if the mode's integer value is outside `0..=65535`.
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

/// Maps a `Command` back to the SDK type `BroadcastMessage`
impl TryFrom<Command> for BroadcastMessage {
    type Error = crate::error::Error;

    /// Encodes a command as SDK broadcast arguments without sending it.
    ///
    /// Car numbers preserve leading-zero padding; numbers that cannot be parsed
    /// as `u16` use zero as their numeric value before applying that padding.
    /// Replay session times are in milliseconds, and force-feedback values retain
    /// their floating-point bit representation.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Validation`] if a chat macro number is
    /// outside `1..=15`.
    ///
    /// Returns [`crate::error::Error::Validation`] if `CameraState` doesn't fit into the broadcast protocol's 16-bit argument
    ///
    /// # Panics
    ///
    /// With overflow checks enabled, panics if car-number padding overflows `u16`.
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

            Command::CameraSetState(camera_state) => {
                let bits = u16::try_from(camera_state.bits()).map_err(|_| {
                    crate::error::Error::Validation {
                        reason: format!(
                            "Camera state bits must fit in 16 bits, got {:#010x}",
                            camera_state.bits()
                        ),
                    }
                })?;

                broadcast_message!(BroadcastMessageKind::CameraSetState, bits)
            }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_message(
        command: Command,
        kind: BroadcastMessageKind,
        var1: u16,
        var2: u16,
        var3: u16,
    ) {
        let message = BroadcastMessage::try_from(command).expect("command should encode");

        assert_eq!(message, BroadcastMessage::new(kind, var1, var2, var3));
    }

    #[test]
    fn camera_switch_position_encodes_to_broadcast_message() {
        assert_message(
            Command::CameraSwitchPosition(12, 3, 4),
            BroadcastMessageKind::CameraSwitchPosition,
            12,
            3,
            4,
        );
    }

    #[test]
    fn replay_position_splits_frame_number_into_words() {
        assert_message(
            Command::ReplaySetPlayPosition(ReplayPositionMode::Current, 0x1234_5678),
            BroadcastMessageKind::ReplaySetPlayPosition,
            encode_mode(ReplayPositionMode::Current),
            0x5678,
            0x1234,
        );
    }

    #[test]
    fn chat_macro_encodes_macro_mode_and_number() {
        assert_message(
            Command::ChatMacro(15),
            BroadcastMessageKind::ChatCommand,
            encode_mode(ChatCommandMode::Macro),
            15,
            0,
        );
    }

    #[test]
    fn chat_macro_rejects_out_of_range_number() {
        let error = BroadcastMessage::try_from(Command::ChatMacro(16))
            .expect_err("macro numbers above 15 must be rejected");

        assert!(matches!(error, crate::error::Error::Validation { .. }));
    }

    #[test]
    fn pit_fuel_encodes_mode_and_value() {
        assert_message(
            Command::Pit(PitCommand::Fuel(8)),
            BroadcastMessageKind::PitCommand,
            encode_mode(PitCommandMode::Fuel),
            8,
            0,
        );
    }

    #[test]
    fn force_feedback_preserves_existing_encoding() {
        let bits = 1.0_f32.to_bits();
        let (low, high) = split_u32_words(bits);

        assert_message(
            Command::ForceFeedback(1.0),
            BroadcastMessageKind::ForceFeedbackCommand,
            encode_mode(ForceFeedbackCommandMode::MaxForce),
            low,
            high,
        );
    }

    #[test]
    fn replay_session_time_splits_time_into_words() {
        assert_message(
            Command::ReplaySearchSessionTime(7, 0x1234_5678),
            BroadcastMessageKind::ReplaySearchSessionTime,
            7,
            0x5678,
            0x1234,
        );
    }
}
