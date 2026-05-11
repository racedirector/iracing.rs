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
    BroadcastMessage as RawBroadcastMessage, CameraState, ChatCommandMode, IRacingSDKError,
    PitCommandMode, ReplayPositionMode, ReplaySearchMode, ReplayStateMode, Result,
    TelemetryCommandMode, VideoCaptureMode, windows::utils::pad_car_number,
};
use {
    windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::WindowsAndMessaging::{HWND_BROADCAST, RegisterWindowMessageW, SendNotifyMessageW},
    },
    windows::core::PCWSTR,
};

const BROADCAST_MESSAGE_NAME: &str = r"IRSDK_BROADCASTMSG";

/// Commands that adjust pit service behavior for the player's car.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitCommand {
    /// Clear all pending pit service selections.
    Clear,
    /// Request a windshield tearoff.
    Tearoff,
    /// Set fuel amount in gallons.
    Fuel(u8),
    /// Set left-front tire pressure in PSI.
    LF(u16),
    /// Set right-front tire pressure in PSI.
    RF(u16),
    /// Set left-rear tire pressure in PSI.
    LR(u16),
    /// Set right-rear tire pressure in PSI.
    RR(u16),
    /// Clear all tire pressure changes.
    ClearTires,
    /// Request fast repair.
    FastRepair,
    /// Clear windshield tearoff request.
    ClearTearoff,
    /// Clear fast repair request.
    ClearFastRepair,
    /// Clear fuel request.
    ClearFuel,
}

impl PitCommand {
    fn encode(self) -> (u16, u16) {
        use PitCommandMode as Id;

        match self {
            PitCommand::Clear => (Id::Clear.into(), 0),
            PitCommand::Tearoff => (Id::Ws.into(), 0),
            PitCommand::Fuel(gal) => (Id::Fuel.into(), gal as u16),
            PitCommand::LF(pressure) => (Id::Lf.into(), pressure),
            PitCommand::RF(pressure) => (Id::Rf.into(), pressure),
            PitCommand::LR(pressure) => (Id::Lr.into(), pressure),
            PitCommand::RR(pressure) => (Id::Rr.into(), pressure),
            PitCommand::ClearTires => (Id::ClearTires.into(), 0),
            PitCommand::FastRepair => (Id::Fr.into(), 0),
            PitCommand::ClearTearoff => (Id::ClearWs.into(), 0),
            PitCommand::ClearFastRepair => (Id::ClearFr.into(), 0),
            PitCommand::ClearFuel => (Id::ClearFuel.into(), 0),
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
    /// Jump to a replay position, with the frame number encoded in `var2`.
    ReplaySetPlayPosition(ReplayPositionMode, u16),
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

type BroadcastMessageFormat = (RawBroadcastMessage, u16, u16, u16);

impl Into<BroadcastMessageFormat> for BroadcastCommand {
    fn into(self) -> BroadcastMessageFormat {
        match self {
            BroadcastCommand::CameraSwitchPosition(position, group, camera) => {
                (RawBroadcastMessage::CamSwitchPos, position, group, camera)
            }
            BroadcastCommand::CameraSwitchNumber(car_number, group, camera) => (
                RawBroadcastMessage::CamSwitchNum,
                pad_car_number(&car_number),
                group,
                camera,
            ),
            BroadcastCommand::CameraSetState(camera_state) => (
                RawBroadcastMessage::CamSetState,
                camera_state.bits() as u16,
                0,
                0,
            ),
            BroadcastCommand::ReplaySetPlaySpeed(speed, slow_motion) => (
                RawBroadcastMessage::ReplaySetPlaySpeed,
                speed as u16,
                slow_motion.into(),
                0,
            ),
            BroadcastCommand::ReplaySetPlayPosition(mode, frame_number) => (
                RawBroadcastMessage::ReplaySetPlayPosition,
                mode.into(),
                frame_number,
                0,
            ),
            BroadcastCommand::ReplaySearch(mode) => {
                (RawBroadcastMessage::ReplaySearch, mode.into(), 0, 0)
            }
            BroadcastCommand::ReplaySetState(mode) => {
                (RawBroadcastMessage::ReplaySetState, mode.into(), 0, 0)
            }
            BroadcastCommand::ReloadAllTextures => (RawBroadcastMessage::ReloadTextures, 0, 0, 0),
            BroadcastCommand::ReloadTextures(car_index) => {
                (RawBroadcastMessage::ReloadTextures, car_index, 0, 0)
            }
            BroadcastCommand::ChatCommand(mode) => {
                (RawBroadcastMessage::ChatCommand, mode.into(), 0, 0)
            }
            BroadcastCommand::ChatCommandMacro(macro_number) => (
                RawBroadcastMessage::ChatCommand,
                ChatCommandMode::Macro.into(),
                macro_number,
                0,
            ),
            BroadcastCommand::PitCommand(pit_command_mode) => {
                let (var1, var2) = Self::encode_pit(pit_command_mode);
                (RawBroadcastMessage::PitCommand, var1, var2, 0)
            }
            BroadcastCommand::TelemetryCommand(mode) => {
                (RawBroadcastMessage::TelemCommand, mode.into(), 0, 0)
            }
            BroadcastCommand::FFBCommand(value) => {
                let bits = value.to_bits();
                (
                    RawBroadcastMessage::FfbCommand,
                    0,
                    (bits & 0xFFFF) as u16,
                    ((bits >> 16) & 0xFFFF) as u16,
                )
            }
            BroadcastCommand::ReplaySearchSessionTime(session_number, session_time_ms) => (
                RawBroadcastMessage::ReplaySearchSessionTime,
                session_number.into(),
                (session_time_ms & 0xFFFF) as u16,
                ((session_time_ms >> 16) & 0xFFFF) as u16,
            ),
            BroadcastCommand::VideoCapture(mode) => {
                (RawBroadcastMessage::VideoCapture, mode.into(), 0, 0)
            }
        }
    }
}

/// Client for sending iRacing broadcast commands over the Win32 broadcast channel.
#[derive(Debug, Default)]
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
        let message: Vec<u16> = crate::windows::wide_string(BROADCAST_MESSAGE_NAME);

        let id = unsafe { RegisterWindowMessageW(PCWSTR::from_raw(message.as_ptr())) };

        if id == 0 {
            return Err(IRacingSDKError::connection_failed(format!(
                "Failed to register broadcast window message '{BROADCAST_MESSAGE_NAME}'"
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
    /// Returns [`IRacingSDKError`] if `SendNotifyMessageW` reports a Win32 error.
    pub fn send_message(&self, message: BroadcastMessageFormat) -> Result<()> {
        let (broadcast_type, var1, var2, var3) = message;

        // Pack the low/high words to match the Windows broadcast contract.
        let wparam_value = (broadcast_type.to_raw() as usize) | ((var1 as usize) << 16);
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
        let encoded: BroadcastMessageFormat =
            BroadcastCommand::CameraSwitchPosition(3, 2, 1).into();
        assert_eq!(encoded, (RawBroadcastMessage::CamSwitchPos, 3, 2, 1));
    }

    #[test]
    fn encodes_camera_switch_number_with_padding() {
        let encoded: BroadcastMessageFormat =
            BroadcastCommand::CameraSwitchNumber("001".to_string(), 4, 5).into();
        assert_eq!(encoded, (RawBroadcastMessage::CamSwitchNum, 3001, 4, 5));
    }

    #[test]
    fn encodes_reload_textures_variants() {
        let reload_all_textures_message: BroadcastMessageFormat =
            BroadcastCommand::ReloadAllTextures.into();

        assert_eq!(
            reload_all_textures_message,
            (RawBroadcastMessage::ReloadTextures, 0, 0, 0)
        );

        let reload_index_textures_message: BroadcastMessageFormat =
            BroadcastCommand::ReloadTextures(7).into();

        assert_eq!(
            reload_index_textures_message,
            (RawBroadcastMessage::ReloadTextures, 7, 0, 0)
        );
    }

    #[test]
    fn encodes_replay_commands() {
        let set_play_speed_message: BroadcastMessageFormat =
            BroadcastCommand::ReplaySetPlaySpeed(2, true).into();

        assert_eq!(
            set_play_speed_message,
            (RawBroadcastMessage::ReplaySetPlaySpeed, 2, 1, 0)
        );

        let set_play_speed_negative_message: BroadcastMessageFormat =
            BroadcastCommand::ReplaySetPlaySpeed(-2, false).into();
        assert_eq!(
            set_play_speed_negative_message,
            (RawBroadcastMessage::ReplaySetPlaySpeed, 0xFFFE, 0, 0)
        );

        let set_play_position_message: BroadcastMessageFormat =
            BroadcastCommand::ReplaySetPlayPosition(ReplayPositionMode::Current, 120).into();
        assert_eq!(
            set_play_position_message,
            (
                RawBroadcastMessage::ReplaySetPlayPosition,
                ReplayPositionMode::Current.into(),
                120,
                0
            )
        );

        let set_search_session_time_message: BroadcastMessageFormat =
            BroadcastCommand::ReplaySearchSessionTime(1, 3400).into();
        assert_eq!(
            set_search_session_time_message,
            (RawBroadcastMessage::ReplaySearchSessionTime, 1, 3400, 0)
        );

        let set_search_session_time_: BroadcastMessageFormat =
            BroadcastCommand::ReplaySearchSessionTime(1, 100_000).into();
        assert_eq!(
            set_search_session_time_,
            (
                RawBroadcastMessage::ReplaySearchSessionTime,
                1,
                0x86A0,
                0x0001
            )
        );
    }

    #[test]
    fn encodes_chat_commands() {
        let begin_chat_command: BroadcastMessageFormat =
            BroadcastCommand::ChatCommand(ChatCommandMode::BeginChat).into();
        assert_eq!(
            begin_chat_command,
            (
                RawBroadcastMessage::ChatCommand,
                ChatCommandMode::BeginChat.into(),
                0,
                0
            )
        );

        let chat_command_macro: BroadcastMessageFormat =
            BroadcastCommand::ChatCommandMacro(9).into();
        assert_eq!(
            chat_command_macro,
            (
                RawBroadcastMessage::ChatCommand,
                ChatCommandMode::Macro.into(),
                9,
                0
            )
        );
    }

    #[test]
    fn encodes_pit_commands() {
        let set_fuel_command: BroadcastMessageFormat =
            BroadcastCommand::PitCommand(PitCommand::Fuel(14)).into();

        assert_eq!(
            set_fuel_command,
            (
                RawBroadcastMessage::PitCommand,
                PitCommandMode::Fuel.into(),
                14,
                0
            )
        );

        let clear_tearoff_command: BroadcastMessageFormat =
            BroadcastCommand::PitCommand(PitCommand::ClearTearoff).into();
        assert_eq!(
            clear_tearoff_command,
            (
                RawBroadcastMessage::PitCommand,
                PitCommandMode::ClearWs.into(),
                0,
                0
            )
        );
    }

    #[test]
    fn encodes_replay_state() {
        let set_replay_state_message: BroadcastMessageFormat =
            BroadcastCommand::ReplaySetState(ReplayStateMode::EraseTape).into();

        assert_eq!(
            set_replay_state_message,
            (
                RawBroadcastMessage::ReplaySetState,
                ReplayStateMode::EraseTape.into(),
                0,
                0
            )
        );
    }

    #[test]
    fn encodes_ffb_max_force_bits() {
        let (_, var1, var2, var3) = BroadcastCommand::FFBCommand(20.9998).into();
        let bits = 20.9998f32.to_bits();
        assert_eq!(var1, 0);
        assert_eq!(var2, (bits & 0xFFFF) as u16);
        assert_eq!(var3, ((bits >> 16) & 0xFFFF) as u16);
    }
}
