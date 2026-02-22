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
    PitCommandMode, ReplayPositionMode, ReplaySearchMode, Result, TelemetryCommandMode,
    VideoCaptureMode,
    windows::utils::pad_car_number,
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
    LF(u8),
    /// Set right-front tire pressure in PSI.
    RF(u8),
    /// Set left-rear tire pressure in PSI.
    LR(u8),
    /// Set right-rear tire pressure in PSI.
    RR(u8),
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
            PitCommand::LF(psi) => (Id::Lf.into(), psi as u16),
            PitCommand::RF(psi) => (Id::Rf.into(), psi as u16),
            PitCommand::LR(psi) => (Id::Lr.into(), psi as u16),
            PitCommand::RR(psi) => (Id::Rr.into(), psi as u16),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BroadcastCommand {
    /// Switch to a specific camera group and camera index for a position.
    CameraSwitchPosition(u8, u8, u8),
    /// Switch to a specific camera group and camera index for a car number.
    CameraSwitchNumber(&'static str, u8, u8),
    /// Apply a new [`CameraState`] bitfield.
    CameraSetState(CameraState),
    /// Set the replay play speed, with an optional slow-motion toggle.
    ReplaySetPlaySpeed(u8, bool),
    /// Jump to a replay position, with the frame number encoded in `var2`.
    ReplaySetPlayPosition(ReplayPositionMode, u16),
    /// Perform a replay search according to the provided mode.
    ReplaySearch(ReplaySearchMode),
    /// Toggle the replay state on or off.
    ReplaySetState,
    /// Reload all textures.
    ReloadAllTextures,
    /// Reload textures for a specific car index.
    ReloadTextures(u8),
    /// Send a chat command.
    ChatCommand(ChatCommandMode),
    /// Send a chat macro by number.
    ChatCommandMacro(u8),
    /// Issue a pit command.
    PitCommand(PitCommand),
    /// Control telemetry recording.
    TelemetryCommand(TelemetryCommandMode),
    /// Send a force-feedback command.
    FFBCommand(u16),
    /// Search a replay to a specific session time.
    ReplaySearchSessionTime(u8, u16),
    /// Control video capture.
    VideoCapture(VideoCaptureMode),
}

impl BroadcastCommand {
    fn encode(self) -> (RawBroadcastMessage, u16, u16, u16) {
        match self {
            BroadcastCommand::CameraSwitchPosition(position, group, camera) => (
                RawBroadcastMessage::CamSwitchPos,
                position.into(),
                group.into(),
                camera.into(),
            ),
            BroadcastCommand::CameraSwitchNumber(car_number, group, camera) => (
                RawBroadcastMessage::CamSwitchNum,
                pad_car_number(car_number),
                group.into(),
                camera.into(),
            ),
            BroadcastCommand::CameraSetState(camera_state) => (
                RawBroadcastMessage::CamSetState,
                camera_state.bits() as u16,
                0,
                0,
            ),
            BroadcastCommand::ReplaySetPlaySpeed(speed, slow_motion) => (
                RawBroadcastMessage::ReplaySetPlaySpeed,
                speed.into(),
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
            BroadcastCommand::ReplaySetState => (RawBroadcastMessage::ReplaySetState, 0, 0, 0),
            BroadcastCommand::ReloadAllTextures => (RawBroadcastMessage::ReloadTextures, 0, 0, 0),
            BroadcastCommand::ReloadTextures(car_index) => {
                (RawBroadcastMessage::ReloadTextures, car_index.into(), 0, 0)
            }
            BroadcastCommand::ChatCommand(mode) => {
                (RawBroadcastMessage::ChatCommand, mode.into(), 0, 0)
            }
            BroadcastCommand::ChatCommandMacro(macro_number) => (
                RawBroadcastMessage::ChatCommand,
                ChatCommandMode::Macro.into(),
                macro_number.into(),
                0,
            ),
            BroadcastCommand::PitCommand(pit_command_mode) => {
                let (var1, var2) = encode_pit(pit_command_mode);
                (RawBroadcastMessage::PitCommand, var1, var2, 0)
            }
            BroadcastCommand::TelemetryCommand(mode) => {
                (RawBroadcastMessage::TelemCommand, mode.into(), 0, 0)
            }
            BroadcastCommand::FFBCommand(_value) => (
                RawBroadcastMessage::FfbCommand,
                0,
                0, // (value * 65536).into(),
                0,
            ),
            BroadcastCommand::ReplaySearchSessionTime(session_number, session_time_ms) => (
                RawBroadcastMessage::ReplaySearchSessionTime,
                session_number.into(),
                session_time_ms,
                0,
            ),
            BroadcastCommand::VideoCapture(mode) => {
                (RawBroadcastMessage::VideoCapture, mode.into(), 0, 0)
            }
        }
    }
}

fn encode_pit(command: PitCommand) -> (u16, u16) {
    command.encode()
}

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
    pub fn send_message(&self, message: BroadcastCommand) -> Result<()> {
        let (broadcast_type, var1, var2, var3) = message.encode();
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
        let encoded = BroadcastCommand::CameraSwitchPosition(3, 2, 1).encode();
        assert_eq!(encoded, (RawBroadcastMessage::CamSwitchPos, 3, 2, 1));
    }

    #[test]
    fn encodes_camera_switch_number_with_padding() {
        let encoded = BroadcastCommand::CameraSwitchNumber("001", 4, 5).encode();
        assert_eq!(encoded, (RawBroadcastMessage::CamSwitchNum, 3001, 4, 5));
    }

    #[test]
    fn encodes_reload_textures_variants() {
        assert_eq!(
            BroadcastCommand::ReloadAllTextures.encode(),
            (RawBroadcastMessage::ReloadTextures, 0, 0, 0)
        );
        assert_eq!(
            BroadcastCommand::ReloadTextures(7).encode(),
            (RawBroadcastMessage::ReloadTextures, 7, 0, 0)
        );
    }

    #[test]
    fn encodes_replay_commands() {
        assert_eq!(
            BroadcastCommand::ReplaySetPlaySpeed(2, true).encode(),
            (RawBroadcastMessage::ReplaySetPlaySpeed, 2, 1, 0)
        );
        assert_eq!(
            BroadcastCommand::ReplaySetPlayPosition(ReplayPositionMode::Current, 120).encode(),
            (
                RawBroadcastMessage::ReplaySetPlayPosition,
                ReplayPositionMode::Current.into(),
                120,
                0
            )
        );
        assert_eq!(
            BroadcastCommand::ReplaySearchSessionTime(1, 3400).encode(),
            (RawBroadcastMessage::ReplaySearchSessionTime, 1, 3400, 0)
        );
    }

    #[test]
    fn encodes_chat_commands() {
        assert_eq!(
            BroadcastCommand::ChatCommand(ChatCommandMode::BeginChat).encode(),
            (
                RawBroadcastMessage::ChatCommand,
                ChatCommandMode::BeginChat.into(),
                0,
                0
            )
        );
        assert_eq!(
            BroadcastCommand::ChatCommandMacro(9).encode(),
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
        assert_eq!(
            BroadcastCommand::PitCommand(PitCommand::Fuel(14)).encode(),
            (RawBroadcastMessage::PitCommand, PitCommandMode::Fuel.into(), 14, 0)
        );
        assert_eq!(
            BroadcastCommand::PitCommand(PitCommand::ClearTearoff).encode(),
            (
                RawBroadcastMessage::PitCommand,
                PitCommandMode::ClearWs.into(),
                0,
                0
            )
        );
    }
}
