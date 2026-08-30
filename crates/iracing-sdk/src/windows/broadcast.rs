//! iRacing broadcast message sender.
//!
//! This module wraps the iRacing broadcast window message contract used to
//! control cameras, replay state, pit options, chat, and capture tools from an
//! external process.
//!
//! # Overview
//!
//! - [`Broadcast`] registers the `IRSDK_BROADCASTMSG` message and sends packed
//!   `WPARAM`/`LPARAM` payloads with `SendNotifyMessageW`.
//! - [`BroadcastCommand`] is the typed command surface that maps to iRacing's
//!   documented broadcast message IDs.
//! - [`PitCommand`] provides typed pit-service subcommands for
//!   [`BroadcastCommand::PitCommand`].
//!
//! # Examples
//!
//! Construct commands in a platform-safe doctest:
//!
//! ```rust
//! # #[cfg(windows)] {
//! use iracing_sdk::windows::{BroadcastCommand, PitCommand};
//!
//! let camera = BroadcastCommand::CameraSwitchPosition(0, 1, 2);
//! let pit = BroadcastCommand::PitCommand(PitCommand::Fuel(8));
//!
//! assert!(matches!(camera, BroadcastCommand::CameraSwitchPosition(0, 1, 2)));
//! assert!(matches!(pit, BroadcastCommand::PitCommand(PitCommand::Fuel(8))));
//! # }
//! ```
//!
//! Send a command to iRacing:
//!
//! ```rust,no_run
//! # #[cfg(windows)] {
//! use iracing_sdk::windows::{Broadcast, BroadcastCommand};
//!
//! let client = Broadcast::new().expect("register iRacing broadcast message");
//! client
//!     .send_message(BroadcastCommand::ReloadAllTextures)
//!     .expect("send broadcast command");
//! # }
//! ```

use crate::{
    IRacingSDKError, PitCommand, Result,
    irsdk::{TelemetryCommandMode, constants::IRSDK_BROADCASTMSGNAME},
    types::irsdk::{
        BroadcastMessage, CameraState, ChatCommandMode, ForceFeedbackCommandMode, PitCommandMode,
        ReloadTexturesMode, ReplayPositionMode, ReplaySearchMode, ReplayStateMode,
        VideoCaptureMode,
    },
    windows::{utils::pad_car_number, wide_string},
};
use {
    windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::WindowsAndMessaging::{HWND_BROADCAST, RegisterWindowMessageW, SendNotifyMessageW},
    },
    windows::core::PCWSTR,
};

impl PitCommand {
    fn encode(self) -> (u16, u16) {
        use PitCommandMode as Id;

        match self {
            PitCommand::Clear => (enum_word(Id::Clear), 0),
            PitCommand::Tearoff => (enum_word(Id::WindshieldTearoff), 0),
            PitCommand::Fuel(gallons) => (enum_word(Id::Fuel), gallons),
            PitCommand::LF(pressure) => (enum_word(Id::LeftFrontTire), pressure),
            PitCommand::RF(pressure) => (enum_word(Id::RightFrontTire), pressure),
            PitCommand::LR(pressure) => (enum_word(Id::LeftRearTire), pressure),
            PitCommand::RR(pressure) => (enum_word(Id::RightRearTire), pressure),
            PitCommand::ClearTires => (enum_word(Id::ClearTires), 0),
            PitCommand::FastRepair => (enum_word(Id::FastRepair), 0),
            PitCommand::ClearTearoff => (enum_word(Id::ClearWindshieldTearoff), 0),
            PitCommand::ClearFastRepair => (enum_word(Id::ClearFastRepair), 0),
            PitCommand::ClearFuel => (enum_word(Id::ClearFuel), 0),
        }
    }
}

/// Messages that can be sent to the iRacing simulation.
///
/// Each variant maps to the documented window message contract in the iRacing
/// SDK. Primitive parameters are passed through as-is and packed into the
/// `WPARAM`/`LPARAM` pairs expected by the simulator.
///
/// # Examples
///
/// ```
/// use iracing_sdk::windows::{BroadcastCommand, PitCommand};
///
/// let _ = BroadcastCommand::CameraSwitchPosition(0, 0, 0);
/// let _ = BroadcastCommand::PitCommand(PitCommand::Fuel(8));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum BroadcastCommand {
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
    ChatCommand(ChatCommandMode),
    /// Send a chat macro by number.
    ChatCommandMacro(u16),
    /// Issue a pit command.
    PitCommand(PitCommand),
    /// Control telemetry recording.
    TelemetryCommand(TelemetryCommandMode),
    /// Send a force-feedback command.
    FFBCommand(f32),
    /// Search a replay to a specific session time.
    ReplaySearchSessionTime(u16, u32),
    /// Control video capture.
    VideoCapture(VideoCaptureMode),
}

impl BroadcastCommand {
    fn encode_pit(command: PitCommand) -> (u16, u16) {
        command.encode()
    }
}

type BroadcastMessageFormat = (BroadcastMessage, u16, u16, u16);

fn split_u32_words(value: u32) -> (u16, u16) {
    ((value & 0xFFFF) as u16, ((value >> 16) & 0xFFFF) as u16)
}

fn enum_word<T>(value: T) -> u16
where
    i32: From<T>,
{
    i32::from(value) as u16
}

impl TryFrom<BroadcastCommand> for BroadcastMessageFormat {
    type Error = IRacingSDKError;

    fn try_from(command: BroadcastCommand) -> std::result::Result<Self, Self::Error> {
        let message = match command {
            BroadcastCommand::CameraSwitchPosition(position, group, camera) => (
                BroadcastMessage::CameraSwitchPosition,
                position,
                group,
                camera,
            ),
            BroadcastCommand::CameraSwitchNumber(car_number, group, camera) => (
                BroadcastMessage::CameraSwitchNumber,
                pad_car_number(&car_number),
                group,
                camera,
            ),
            BroadcastCommand::CameraSetState(camera_state) => (
                BroadcastMessage::CameraSetState,
                camera_state.bits() as u16,
                0,
                0,
            ),
            BroadcastCommand::ReplaySetPlaySpeed(speed, slow_motion) => (
                BroadcastMessage::ReplaySetPlaySpeed,
                speed as u16,
                slow_motion.into(),
                0,
            ),
            BroadcastCommand::ReplaySetPlayPosition(mode, frame_number) => {
                let (low, high) = split_u32_words(frame_number);
                (
                    BroadcastMessage::ReplaySetPlayPosition,
                    enum_word(mode),
                    low,
                    high,
                )
            }
            BroadcastCommand::ReplaySearch(mode) => {
                (BroadcastMessage::ReplaySearch, enum_word(mode), 0, 0)
            }
            BroadcastCommand::ReplaySetState(mode) => {
                (BroadcastMessage::ReplaySetState, enum_word(mode), 0, 0)
            }
            BroadcastCommand::ReloadAllTextures => (
                BroadcastMessage::ReloadTextures,
                enum_word(ReloadTexturesMode::All),
                0,
                0,
            ),
            BroadcastCommand::ReloadTextures(car_index) => (
                BroadcastMessage::ReloadTextures,
                enum_word(ReloadTexturesMode::CarIndex),
                car_index,
                0,
            ),
            BroadcastCommand::ChatCommand(mode) => {
                (BroadcastMessage::ChatCommand, enum_word(mode), 0, 0)
            }
            BroadcastCommand::ChatCommandMacro(macro_number) => {
                if !(1..=15).contains(&macro_number) {
                    return Err(IRacingSDKError::Parse {
                        context: "chat macro validation".to_string(),
                        details: format!("macro id must be in range 1..=15, got {macro_number}"),
                    });
                }

                (
                    BroadcastMessage::ChatCommand,
                    enum_word(ChatCommandMode::Macro),
                    macro_number,
                    0,
                )
            }
            BroadcastCommand::PitCommand(pit_command_mode) => {
                let (var1, var2) = BroadcastCommand::encode_pit(pit_command_mode);
                (BroadcastMessage::PitCommand, var1, var2, 0)
            }
            BroadcastCommand::TelemetryCommand(mode) => {
                (BroadcastMessage::TelemetryCommand, enum_word(mode), 0, 0)
            }
            BroadcastCommand::FFBCommand(value) => {
                let bits = value.to_bits();
                let (low, high) = split_u32_words(bits);
                (
                    BroadcastMessage::ForceFeedbackCommand,
                    enum_word(ForceFeedbackCommandMode::MaxForce),
                    low,
                    high,
                )
            }
            BroadcastCommand::ReplaySearchSessionTime(session_number, session_time_ms) => {
                let (low, high) = split_u32_words(session_time_ms);
                (
                    BroadcastMessage::ReplaySearchSessionTime,
                    session_number,
                    low,
                    high,
                )
            }
            BroadcastCommand::VideoCapture(mode) => {
                (BroadcastMessage::VideoCapture, enum_word(mode), 0, 0)
            }
        };

        Ok(message)
    }
}

/// Client for sending iRacing broadcast commands over the Win32 broadcast channel.
#[derive(Debug)]
pub struct Broadcast {
    message_id: u32,
}

impl Broadcast {
    /// Create a new broadcast client by registering the iRacing message ID.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError`] if `RegisterWindowMessageW` fails.
    pub fn new() -> Result<Self> {
        let message: Vec<u16> = wide_string(IRSDK_BROADCASTMSGNAME);

        let id = unsafe { RegisterWindowMessageW(PCWSTR::from_raw(message.as_ptr())) };

        if id == 0 {
            return Err(IRacingSDKError::connection_failed(format!(
                "Failed to register broadcast window message '{IRSDK_BROADCASTMSGNAME}'"
            )));
        }

        Ok(Self { message_id: id })
    }

    /// Send a typed broadcast message to iRacing.
    ///
    /// The command is packed into the `WPARAM`/`LPARAM` format expected by the
    /// official iRacing SDK and dispatched via `HWND_BROADCAST`.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError`] if the command cannot be encoded or if
    /// `SendNotifyMessageW` reports a Win32 error.
    pub fn send_message(&self, message: BroadcastCommand) -> Result<()> {
        let (broadcast_type, var1, var2, var3) = message.try_into()?;

        // Pack the low/high words to match the Windows broadcast contract.
        let wparam_value = (i32::from(broadcast_type) as usize) | ((var1 as usize) << 16);
        let lparam_value = i32::from(var2) | (i32::from(var3) << 16);

        unsafe {
            // Safety: iRacing expects these messages to be delivered to
            // HWND_BROADCAST using the ID obtained from RegisterWindowMessageW.
            // All parameter packing matches the documented protocol, so the
            // Win32 API receives well-formed data.
            SendNotifyMessageW(
                HWND_BROADCAST,
                self.message_id,
                WPARAM(wparam_value),
                LPARAM(lparam_value as isize),
            )
            .map_err(|e| IRacingSDKError::windows_api_error("SendNotifyMessageW", e))
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn encodes_camera_switch_position() {
        let encoded: BroadcastMessageFormat = BroadcastCommand::CameraSwitchPosition(3, 2, 1)
            .try_into()
            .unwrap();
        assert_eq!(encoded, (BroadcastMessage::CameraSwitchPosition, 3, 2, 1));
    }

    #[test]
    fn encodes_camera_switch_number_with_padding() {
        let encoded: BroadcastMessageFormat =
            BroadcastCommand::CameraSwitchNumber("001".to_string(), 4, 5)
                .try_into()
                .unwrap();
        assert_eq!(encoded, (BroadcastMessage::CameraSwitchNumber, 3001, 4, 5));
    }

    #[test]
    fn encodes_reload_textures_variants() {
        let reload_all_textures_message: BroadcastMessageFormat =
            BroadcastCommand::ReloadAllTextures.try_into().unwrap();

        assert_eq!(
            reload_all_textures_message,
            (
                BroadcastMessage::ReloadTextures,
                enum_word(ReloadTexturesMode::All),
                0,
                0
            )
        );

        let reload_index_textures_message: BroadcastMessageFormat =
            BroadcastCommand::ReloadTextures(7).try_into().unwrap();

        assert_eq!(
            reload_index_textures_message,
            (
                BroadcastMessage::ReloadTextures,
                enum_word(ReloadTexturesMode::CarIndex),
                7,
                0
            )
        );
    }

    #[test]
    fn encodes_replay_commands() {
        let set_play_speed_message: BroadcastMessageFormat =
            BroadcastCommand::ReplaySetPlaySpeed(2, true)
                .try_into()
                .unwrap();

        assert_eq!(
            set_play_speed_message,
            (BroadcastMessage::ReplaySetPlaySpeed, 2, 1, 0)
        );

        let set_play_speed_negative_message: BroadcastMessageFormat =
            BroadcastCommand::ReplaySetPlaySpeed(-2, false)
                .try_into()
                .unwrap();
        assert_eq!(
            set_play_speed_negative_message,
            (BroadcastMessage::ReplaySetPlaySpeed, 0xFFFE, 0, 0)
        );

        let set_play_position_message: BroadcastMessageFormat =
            BroadcastCommand::ReplaySetPlayPosition(ReplayPositionMode::Current, 100_000)
                .try_into()
                .unwrap();
        assert_eq!(
            set_play_position_message,
            (
                BroadcastMessage::ReplaySetPlayPosition,
                enum_word(ReplayPositionMode::Current),
                0x86A0,
                0x0001
            )
        );

        let set_search_session_time_message: BroadcastMessageFormat =
            BroadcastCommand::ReplaySearchSessionTime(1, 3400)
                .try_into()
                .unwrap();
        assert_eq!(
            set_search_session_time_message,
            (BroadcastMessage::ReplaySearchSessionTime, 1, 3400, 0)
        );

        let set_search_session_time_: BroadcastMessageFormat =
            BroadcastCommand::ReplaySearchSessionTime(1, 100_000)
                .try_into()
                .unwrap();
        assert_eq!(
            set_search_session_time_,
            (BroadcastMessage::ReplaySearchSessionTime, 1, 0x86A0, 0x0001)
        );
    }

    #[test]
    fn encodes_chat_commands() {
        let begin_chat_command: BroadcastMessageFormat =
            BroadcastCommand::ChatCommand(ChatCommandMode::BeginChat)
                .try_into()
                .unwrap();
        assert_eq!(
            begin_chat_command,
            (
                BroadcastMessage::ChatCommand,
                enum_word(ChatCommandMode::BeginChat),
                0,
                0
            )
        );

        let chat_command_macro: BroadcastMessageFormat =
            BroadcastCommand::ChatCommandMacro(9).try_into().unwrap();
        assert_eq!(
            chat_command_macro,
            (
                BroadcastMessage::ChatCommand,
                enum_word(ChatCommandMode::Macro),
                9,
                0
            )
        );
    }

    #[test]
    fn rejects_invalid_chat_macro() {
        let err = BroadcastMessageFormat::try_from(BroadcastCommand::ChatCommandMacro(16))
            .expect_err("invalid chat macro should fail encoding");

        assert!(matches!(
            err,
            IRacingSDKError::Parse {
                context,
                details
            } if context == "chat macro validation"
                && details == "macro id must be in range 1..=15, got 16"
        ));
    }

    #[test]
    fn encodes_pit_commands() {
        let set_fuel_command: BroadcastMessageFormat =
            BroadcastCommand::PitCommand(PitCommand::Fuel(14))
                .try_into()
                .unwrap();

        assert_eq!(
            set_fuel_command,
            (
                BroadcastMessage::PitCommand,
                enum_word(PitCommandMode::Fuel),
                14,
                0
            )
        );

        let clear_tearoff_command: BroadcastMessageFormat =
            BroadcastCommand::PitCommand(PitCommand::ClearTearoff)
                .try_into()
                .unwrap();
        assert_eq!(
            clear_tearoff_command,
            (
                BroadcastMessage::PitCommand,
                enum_word(PitCommandMode::ClearWindshieldTearoff),
                0,
                0
            )
        );
    }

    #[test]
    fn encodes_replay_state() {
        let set_replay_state_message: BroadcastMessageFormat =
            BroadcastCommand::ReplaySetState(ReplayStateMode::EraseTape)
                .try_into()
                .unwrap();

        assert_eq!(
            set_replay_state_message,
            (
                BroadcastMessage::ReplaySetState,
                enum_word(ReplayStateMode::EraseTape),
                0,
                0
            )
        );
    }

    #[test]
    fn encodes_ffb_max_force_bits() {
        let (_, var1, var2, var3) = BroadcastCommand::FFBCommand(20.9998).try_into().unwrap();
        let bits = 20.9998f32.to_bits();
        assert_eq!(var1, enum_word(ForceFeedbackCommandMode::MaxForce));
        assert_eq!(var2, (bits & 0xFFFF) as u16);
        assert_eq!(var3, ((bits >> 16) & 0xFFFF) as u16);
    }
}
