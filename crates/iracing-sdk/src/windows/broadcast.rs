use {
    windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::WindowsAndMessaging::{HWND_BROADCAST, RegisterWindowMessageW, SendNotifyMessageW},
    },
    windows::core::PCWSTR,
};

const BROADCAST_MESSAGE_NAME: &str = r"IRSDK_BROADCASTMSG";

/// Identifiers for broadcast messages recognized by the iRacing simulator.
#[repr(u32)]
pub enum BroadcastMessageType {
    /// Switch to a camera by position index.
    CameraSwitchPosition = 0,
    /// Switch to a camera by car number.
    CameraSwitchNumber,
    /// Update the camera state bitfield.
    CameraSetState,
    /// Change replay playback speed.
    ReplaySetPlaySpeed,
    /// Move to a specific replay position.
    ReplaySetPlayPosition,
    /// Perform a replay search.
    ReplaySearch,
    /// Toggle the replay state.
    ReplaySetState,
    /// Reload one or more textures.
    ReloadTextures,
    /// Issue a chat command.
    ChatCommand,
    /// Issue a pit command.
    PitCommand,
    /// Control telemetry capture.
    TelemetryCommand,
    /// Send a force-feedback command.
    FFBCommand,
    /// Search to a session-relative time.
    ReplaySearchSessionTime,
    /// Control screenshot or capture recording.
    VideoCapture,
}

impl From<BroadcastMessageType> for usize {
    fn from(value: BroadcastMessageType) -> Self {
        value as u32 as usize
    }
}

pub trait BroadcastMessageProvider {
    fn to_message(self) -> (BroadcastMessageType, u16, u16, u16);
}

pub struct Broadcast {
    message_id: u32,
}

impl Broadcast {
    pub fn new() -> Result<Self> {
        let message: Vec<u16> = wide_string(BROADCAST_MESSAGE_NAME);

        let id = unsafe { RegisterWindowMessageW(PCWSTR::from_raw(message.as_ptr())) };

        if id == 0 {
            return Err(BroadcastError::connection_failed(format!(
                "Failed to register broadcast window message '{BROADCAST_MESSAGE_NAME}'"
            )));
        }
    }

    pub fn send_message<M: BroadcastMessageProvider>(&self, message: M) -> Result<()> {
        let (broadcast_type, var1, var2, var3) = message.to_message();
        // Pack the low/high words to match the Windows broadcast contract.
        let wparam_value = broadcast_type as usize | ((var1 as usize) << 16);
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
            .map_err(|e| BroadcastError::windows_api_error("SendNotifyMessageW", e))
        }
    }
}
