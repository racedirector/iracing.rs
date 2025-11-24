pub enum BroadcastMessageType {
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
