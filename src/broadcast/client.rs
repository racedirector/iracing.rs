use std::convert::TryInto;
use std::ffi::OsStr;
use std::io::{self, Result as IOResult};
use std::num::NonZeroU32;
use std::os::windows::ffi::OsStrExt;
use winapi::shared::minwindef::{LPARAM, WPARAM};
use winapi::um::winuser::{RegisterWindowMessageW, SendNotifyMessageW, HWND_BROADCAST};

use crate::broadcast::message::{
    BroadcastMessageType, ChatCommandMode, PitCommandMode, ReplayPositionMode, ReplaySearchMode,
    TelemetryCommandMode, VideoCaptureMode,
};
use crate::broadcast::util::pad_car_number;
use crate::states::CameraState;

const BROADCAST_MESSAGE_NAME: &str = r"IRSDK_BROADCASTMSG";

pub trait BroadcastMessageProvider {
    fn to_message(self) -> (BroadcastMessageType, u16, u16, u16);
}

///
/// Messages that can be sent to the iRacing simulation.
///
/// # Examples
///
/// ```
/// use iracing::broadcast::BroadcastMessage;
///
/// let _ = BroadcastMessage::CameraSwitchPosition(0, 0, 0);
/// let _ = BroadcastMessage::CameraSwitchNumber("001", 0, 0);
/// ```
pub enum BroadcastMessage {
    CameraSwitchPosition(u8, u8, u8),
    CameraSwitchNumber(&'static str, u8, u8),
    CameraSetState(CameraState),
    ReplaySetPlaySpeed(u8, bool),
    ReplaySetPlayPosition(ReplayPositionMode, u16),
    ReplaySearch(ReplaySearchMode),
    ReplaySetState,
    ReloadAllTextures,
    ReloadTextures(u8),
    ChatCommand(ChatCommandMode),
    ChatCommandMacro(u8),
    PitCommand(PitCommandMode),
    TelemetryCommand(TelemetryCommandMode),
    FFBCommand(u16),
    ReplaySearchSessionTime(u8, u16),
    VideoCapture(VideoCaptureMode),
}

impl BroadcastMessageProvider for BroadcastMessage {
    fn to_message(self) -> (BroadcastMessageType, u16, u16, u16) {
        match self {
            BroadcastMessage::CameraSwitchPosition(position, group, camera) => (
                BroadcastMessageType::CameraSwitchPosition,
                position.into(),
                group.into(),
                camera.into(),
            ),
            BroadcastMessage::CameraSwitchNumber(car_number, group, camera) => (
                BroadcastMessageType::CameraSwitchNumber,
                pad_car_number(&car_number),
                group.into(),
                camera.into(),
            ),
            BroadcastMessage::CameraSetState(camera_state) => (
                BroadcastMessageType::CameraSetState,
                camera_state.bits().try_into().unwrap(),
                0,
                0,
            ),
            BroadcastMessage::ReplaySetPlaySpeed(speed, slow_motion) => (
                BroadcastMessageType::ReplaySetPlaySpeed,
                speed.into(),
                slow_motion.into(),
                0,
            ),
            BroadcastMessage::ReplaySetPlayPosition(mode, frame_number) => (
                BroadcastMessageType::ReplaySetPlayPosition,
                mode.into(),
                frame_number.into(),
                0,
            ),
            BroadcastMessage::ReplaySearch(mode) => {
                (BroadcastMessageType::ReplaySearch, mode.into(), 0, 0)
            }
            BroadcastMessage::ReplaySetState => (BroadcastMessageType::ReplaySetState, 0, 0, 0),
            BroadcastMessage::ReloadAllTextures => (BroadcastMessageType::ReloadTextures, 0, 0, 0),
            BroadcastMessage::ReloadTextures(car_index) => {
                (BroadcastMessageType::ReloadTextures, car_index.into(), 0, 0)
            }
            BroadcastMessage::ChatCommand(mode) => {
                (BroadcastMessageType::ChatCommand, mode.into(), 0, 0)
            }
            BroadcastMessage::ChatCommandMacro(macro_number) => (
                BroadcastMessageType::ChatCommand,
                ChatCommandMode::Macro.into(),
                macro_number.into(),
                0,
            ),
            BroadcastMessage::PitCommand(pit_command_mode) => {
                let (var1, var2) = pit_command_mode.encode();
                (BroadcastMessageType::PitCommand, var1, var2, 0)
            }
            BroadcastMessage::TelemetryCommand(mode) => {
                (BroadcastMessageType::TelemetryCommand, mode.into(), 0, 0)
            }
            BroadcastMessage::FFBCommand(_value) => (
                BroadcastMessageType::FFBCommand,
                0,
                0, // (value * 65536).into(),
                0,
            ),
            BroadcastMessage::ReplaySearchSessionTime(session_number, session_time_ms) => (
                BroadcastMessageType::ReplaySearchSessionTime,
                session_number.into(),
                session_time_ms,
                0,
            ),
            BroadcastMessage::VideoCapture(mode) => {
                (BroadcastMessageType::VideoCapture, mode.into(), 0, 0)
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Client {
    message_id: NonZeroU32,
}

impl Client {
    pub fn new() -> IOResult<Client> {
        let message: Vec<u16> = OsStr::new(BROADCAST_MESSAGE_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let id = unsafe { RegisterWindowMessageW(message.as_ptr()) };
        let message_id = NonZeroU32::new(id).ok_or_else(io::Error::last_os_error)?;
        Ok(Client { message_id })
    }

    pub fn send_message<M: BroadcastMessageProvider>(&self, message: M) {
        let (broadcast_type, var1, var2, var3) = message.to_message();
        // Pack the low/high words to match the Windows broadcast contract.
        let wparam: WPARAM = (broadcast_type as WPARAM) | ((var1 as WPARAM) << 16);
        let lparam: LPARAM = (var2 as LPARAM) | ((var3 as LPARAM) << 16);
        unsafe { SendNotifyMessageW(HWND_BROADCAST, self.message_id.into(), wparam, lparam) };
    }
}

#[cfg(test)]
mod tests {
    use crate::broadcast::message::PitCommandMode;

    use super::*;
    use crate::broadcast::BroadcastMessage;

    #[test]
    fn test_broadcast() {
        let broadcast = Client::new();
        assert!(broadcast.is_ok());
    }

    #[test]
    fn test_message() {
        let broadcast = Client::new().expect("Could not register broadcast client");
        broadcast.send_message(BroadcastMessage::PitCommand(PitCommandMode::Tearoff));
    }
}
