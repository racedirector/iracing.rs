use iracing_irsdk::{
    CameraState, ChatCommandMode, PitCommand, ReplayPositionMode, ReplaySearchMode,
    ReplayStateMode, TelemetryCommandMode, VideoCaptureMode,
};

/// Messages that can be sent to the iRacing simulation.
///
/// Each variant maps to the documented window message contract in the iRacing
/// SDK. Primitive parameters are passed through as-is and packed into the
/// `WPARAM`/`LPARAM` pairs expected by the simulator.
///
/// # Examples
///
/// ```
/// pub use iracing_irsdk::PitCommand;
/// pub use client::Client;
/// pub use command::Command;
/// pub use error::{BroadcastError, Result};
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
