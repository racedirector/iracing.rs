use std::convert::TryInto;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use winapi::shared::minwindef::{LPARAM, WPARAM};
use winapi::um::winuser::{RegisterWindowMessageW, SendNotifyMessageW, HWND_BROADCAST};

use crate::states::CameraState;

const BROADCAST_MESSAGE_NAME: &str = r"IRSDK_BROADCASTMSG";

///
/// Replay Position Mode
///
#[repr(u16)]
pub enum ReplayPositionMode {
    Begin = 0,
    Current,
    End,
}

impl From<ReplayPositionMode> for u16 {
    fn from(mode: ReplayPositionMode) -> Self {
        mode as u16
    }
}

///
/// Replay Search Mode
///
#[repr(u16)]
pub enum ReplaySearchMode {
    ToStart = 0,
    ToEnd,
    PreviousSession,
    NextSession,
    PreviousLap,
    NextLap,
    PreviousFrame,
    NextFrame,
    PreviousIncident,
    NextIncident,
}

impl From<ReplaySearchMode> for u16 {
    fn from(mode: ReplaySearchMode) -> Self {
        mode as u16
    }
}

///
/// Telemetry Command Mode
///
#[repr(u16)]
pub enum TelemetryCommandMode {
    Stop = 0,
    Start,
    Restart,
}

impl From<TelemetryCommandMode> for u16 {
    fn from(mode: TelemetryCommandMode) -> Self {
        mode as u16
    }
}

///
/// Chat Command Mode
///
#[repr(u16)]
pub enum ChatCommandMode {
    Macro = 0,
    Begin,
    Reply,
    Cancel,
}

impl From<ChatCommandMode> for u16 {
    fn from(mode: ChatCommandMode) -> Self {
        mode as u16
    }
}

///
/// Pit Command Mode
///
pub enum PitCommandMode {
    Clear,
    Tearoff,
    Fuel(u8),
    LF(u8),
    RF(u8),
    LR(u8),
    RR(u8),
    ClearTires,
    FastRepair,
    ClearTearoff,
    ClearFastRepair,
    ClearFuel,
}

impl PitCommandMode {
    /// Encode into (var1, var2) words as expected by the broadcast API.
    pub fn encode(self) -> (u16, u16) {
        match self {
            PitCommandMode::Clear => (0, 0),
            PitCommandMode::Tearoff => (1, 0),
            PitCommandMode::Fuel(level) => (2, level as u16),
            PitCommandMode::LF(pressure) => (3, pressure as u16),
            PitCommandMode::RF(pressure) => (4, pressure as u16),
            PitCommandMode::LR(pressure) => (5, pressure as u16),
            PitCommandMode::RR(pressure) => (6, pressure as u16),
            PitCommandMode::ClearTires => (7, 0),
            PitCommandMode::FastRepair => (8, 0),
            PitCommandMode::ClearTearoff => (9, 0),
            PitCommandMode::ClearFastRepair => (10, 0),
            PitCommandMode::ClearFuel => (11, 0),
        }
    }
}

///
/// Video Capture Mode
///
#[repr(u16)]
pub enum VideoCaptureMode {
    ScreenShot = 0,
    StartCapture,
    EndCapture,
    ToggleCapture,
    ShowTimer,
    HideTimer,
}

impl From<VideoCaptureMode> for u16 {
    fn from(mode: VideoCaptureMode) -> Self {
        mode as u16
    }
}

enum BroadcastMessageType {
    CameraSwitchPosition = 0,
    CameraSwitchNumber,
    CameraSetState,
    ReplaySetPlaySpeed,
    ReplaySetPlayPosition,
    ReplaySearch,
    ReplaySetState,
    ReloadTextures,
    ChatCommand,
    PitCommand,
    TelemetryCommand,
    FFBCommand,
    ReplaySearchSessionTime,
    VideoCapture,
}

trait BroadcastMessageProvider {
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
    CameraSwitchNumber(String, u8, u8),
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

fn pad_car_number(s: &str) -> u16 {
    let bytes = s.as_bytes();
    let len = bytes.len();

    // Count leading zeros without allocating
    let mut zeros = 0usize;
    for &b in bytes {
        if b == b'0' {
            zeros += 1;
        } else {
            break;
        }
    }

    // If the entire string was zeros, subtract 1
    if zeros > 0 && zeros == len {
        zeros -= 1;
    }

    // Parse the numeric value (leading zeros are fine)
    let num: u16 = s.parse().unwrap();

    if zeros > 0 {
        let num_place = if num > 99 {
            3
        } else if num > 9 {
            2
        } else {
            1
        };

        num + 1000 * (num_place + zeros as u16)
    } else {
        num
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Broadcast {
    message_id: u32,
}

impl Broadcast {
    pub fn new() -> Broadcast {
        let wide: Vec<u16> = OsStr::new(BROADCAST_MESSAGE_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        Broadcast {
            message_id: unsafe { RegisterWindowMessageW(wide.as_ptr()) },
        }
    }

    pub fn send_message(&self, message: BroadcastMessage) {
        let (broadcast_type, var1, var2, var3) = message.to_message();
        // Pack the low/high words to match the Windows broadcast contract.
        let wparam: WPARAM = (broadcast_type as WPARAM) | ((var1 as WPARAM) << 16);
        let lparam: LPARAM = (var2 as LPARAM) | ((var3 as LPARAM) << 16);
        unsafe { SendNotifyMessageW(HWND_BROADCAST, self.message_id, wparam, lparam) };
    }
}
