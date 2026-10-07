use iracing_broadcast_sdk::{
    CameraState, ChatCommandMode, Command as BroadcastCommand, PitCommand, ReplayPositionMode,
    ReplaySearchMode, ReplayStateMode, TelemetryCommandMode, VideoCaptureMode,
};

use crate::parser::{camera_state_parser, replay_position_parser, replay_search_parser};

#[derive(clap::Args, Debug, Clone, PartialEq)]
#[group(required = true, multiple = false)]
pub struct CameraStateArgs {
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..=u16::MAX as i64))]
    raw_bits: Option<u32>,

    #[arg(long = "flag", value_parser = camera_state_parser())]
    flags: Vec<CameraState>,
}

impl From<CameraStateArgs> for CameraState {
    /// Build a camera state from raw bits or the union of named flags.
    ///
    /// Raw bits, including unknown flags, are retained unchanged; the CLI parser
    /// limits them to 16 bits. Repeated named flags have no additional effect.
    fn from(args: CameraStateArgs) -> Self {
        if let Some(raw_bits) = args.raw_bits {
            return CameraState::from_bits_retain(raw_bits);
        }

        args.flags
            .into_iter()
            .fold(CameraState::empty(), |state, flag| state.union(flag))
    }
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum CameraCommand {
    /// Switch the camera position
    SwitchPosition {
        #[arg(long)]
        position: u16,
        #[arg(long)]
        group: u16,
        #[arg(long, default_value_t = 0)]
        camera: u16,
    },
    /// Switch the camera number
    SwitchNumber {
        #[arg(long)]
        car_number: String,
        #[arg(long)]
        group: u16,
        #[arg(long, default_value_t = 0)]
        camera: u16,
    },
    /// Set the camera state
    SetState(CameraStateArgs),
}

impl From<CameraCommand> for BroadcastCommand {
    /// Build a camera broadcast command, preserving the supplied camera selectors.
    ///
    fn from(value: CameraCommand) -> Self {
        match value {
            CameraCommand::SwitchPosition {
                position,
                group,
                camera,
            } => Self::CameraSwitchPosition(position, group, camera),
            CameraCommand::SwitchNumber {
                car_number,
                group,
                camera,
            } => Self::CameraSwitchNumber(car_number, group, camera),
            CameraCommand::SetState(args) => Self::CameraSetState(args.into()),
        }
    }
}

#[derive(clap::Subcommand, Debug, Clone, PartialEq)]
pub enum ReplayCommand {
    /// Set replay play speed
    SetPlaySpeed {
        #[arg(long)]
        speed: i16,
        #[arg(long, default_value_t = false)]
        slow_motion: bool,
    },
    /// Search the replay for the given mode.
    Search {
        #[arg(value_parser = replay_search_parser())]
        mode: ReplaySearchMode,
    },
    /// Set the replay play head
    SetPlayPosition {
        #[arg(value_parser = replay_position_parser())]
        mode: ReplayPositionMode,
        #[arg(long)]
        frame: u32,
    },
    /// Erase replay tape
    Erase,
    /// Search for a provided session time
    SearchSessionTime {
        #[arg(long)]
        session: u16,
        #[arg(long)]
        time_ms: u32,
    },
    Normal,
    Slow16,
    /// Pause the replay
    Pause,
}

impl From<ReplayCommand> for BroadcastCommand {
    /// Build a replay broadcast command, expanding playback shortcuts.
    ///
    /// `Normal`, `Slow16`, and `Pause` select speed/slow-motion pairs `(1, false)`,
    /// `(16, true)`, and `(0, false)`, respectively. Session times remain in milliseconds.
    fn from(value: ReplayCommand) -> Self {
        match value {
            ReplayCommand::SetPlaySpeed { speed, slow_motion } => {
                BroadcastCommand::ReplaySetPlaySpeed(speed, slow_motion)
            }
            ReplayCommand::Search { mode } => BroadcastCommand::ReplaySearch(mode.into()),
            ReplayCommand::SetPlayPosition { mode, frame } => {
                BroadcastCommand::ReplaySetPlayPosition(mode.into(), frame)
            }
            ReplayCommand::Erase => BroadcastCommand::ReplaySetState(ReplayStateMode::EraseTape),
            ReplayCommand::SearchSessionTime { session, time_ms } => {
                BroadcastCommand::ReplaySearchSessionTime(session, time_ms)
            }
            ReplayCommand::Normal => BroadcastCommand::ReplaySetPlaySpeed(1, false),
            ReplayCommand::Slow16 => BroadcastCommand::ReplaySetPlaySpeed(16, true),
            ReplayCommand::Pause => BroadcastCommand::ReplaySetPlaySpeed(0, false),
        }
    }
}

#[derive(clap::Subcommand, Debug, Clone, PartialEq)]
pub enum PitCliCommand {
    Clear,
    Fuel { gallons: u16 },
    Lf { psi: u16 },
    Rf { psi: u16 },
    Lr { psi: u16 },
    Rr { psi: u16 },
    ClearTires,
    Ws,
    Fr,
    ClearWs,
    ClearFr,
    ClearFuel,
}

impl From<PitCliCommand> for BroadcastCommand {
    /// Build a pit-service broadcast command, passing numeric values through unchanged.
    ///
    /// `Ws` requests a windshield tearoff and `Fr` requests a fast repair;
    /// the corresponding clear commands cancel those requests.
    fn from(value: PitCliCommand) -> Self {
        match value {
            PitCliCommand::Clear => BroadcastCommand::Pit(PitCommand::Clear),
            PitCliCommand::Fuel { gallons } => BroadcastCommand::Pit(PitCommand::Fuel(gallons)),
            PitCliCommand::Lf { psi } => BroadcastCommand::Pit(PitCommand::LF(psi)),
            PitCliCommand::Rf { psi } => BroadcastCommand::Pit(PitCommand::RF(psi)),
            PitCliCommand::Lr { psi } => BroadcastCommand::Pit(PitCommand::LR(psi)),
            PitCliCommand::Rr { psi } => BroadcastCommand::Pit(PitCommand::RR(psi)),
            PitCliCommand::ClearTires => BroadcastCommand::Pit(PitCommand::ClearTires),
            PitCliCommand::Ws => BroadcastCommand::Pit(PitCommand::Tearoff),
            PitCliCommand::Fr => BroadcastCommand::Pit(PitCommand::FastRepair),
            PitCliCommand::ClearWs => BroadcastCommand::Pit(PitCommand::ClearTearoff),
            PitCliCommand::ClearFr => BroadcastCommand::Pit(PitCommand::ClearFastRepair),
            PitCliCommand::ClearFuel => BroadcastCommand::Pit(PitCommand::ClearFuel),
        }
    }
}

#[derive(clap::Subcommand, Debug, Clone, PartialEq)]
pub enum ChatCommand {
    /// Cancel chat input
    Cancel,
    /// Reply to the last received message
    Reply,
    /// Open chat input
    Begin,
    /// Send chat macro
    Macro {
        #[arg(value_parser = clap::value_parser!(u16).range(1..=15))]
        index: u16,
    },
}

impl From<ChatCommand> for BroadcastCommand {
    /// Build a chat broadcast command without validating the macro index.
    ///
    /// The CLI parser and SDK message encoder enforce the macro range `1..=15`.
    fn from(value: ChatCommand) -> Self {
        match value {
            ChatCommand::Cancel => Self::Chat(ChatCommandMode::Cancel),
            ChatCommand::Reply => Self::Chat(ChatCommandMode::Reply),
            ChatCommand::Begin => Self::Chat(ChatCommandMode::BeginChat),
            ChatCommand::Macro { index } => Self::ChatMacro(index),
        }
    }
}

#[derive(clap::Subcommand, Debug, Clone, PartialEq)]
pub enum TextureCommand {
    ReloadAll,
    ReloadCar { car_idx: u16 },
}

impl From<TextureCommand> for BroadcastCommand {
    /// Build a texture-reload command targeting all cars or the supplied car index.
    fn from(value: TextureCommand) -> Self {
        match value {
            TextureCommand::ReloadAll => Self::ReloadAllTextures,
            TextureCommand::ReloadCar { car_idx } => Self::ReloadTextures(car_idx),
        }
    }
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum TelemetryCommand {
    Start,
    Stop,
    Restart,
}

impl From<TelemetryCommand> for BroadcastCommand {
    /// Build a broadcast command to control disk telemetry recording.
    fn from(value: TelemetryCommand) -> Self {
        match value {
            TelemetryCommand::Stop => Self::Telemetry(TelemetryCommandMode::Stop),
            TelemetryCommand::Start => Self::Telemetry(TelemetryCommandMode::Start),
            TelemetryCommand::Restart => Self::Telemetry(TelemetryCommandMode::Restart),
        }
    }
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum ForceFeedbackCommand {
    MaxForce { nm: f32 },
}

impl From<ForceFeedbackCommand> for BroadcastCommand {
    fn from(value: ForceFeedbackCommand) -> Self {
        match value {
            ForceFeedbackCommand::MaxForce { nm } => Self::ForceFeedback(nm),
        }
    }
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum VideoCommand {
    Screenshot,
    Start,
    Stop,
    Toggle,
    ShowTimer,
    HideTimer,
}

impl From<VideoCommand> for BroadcastCommand {
    fn from(value: VideoCommand) -> Self {
        match value {
            VideoCommand::Screenshot => Self::VideoCapture(VideoCaptureMode::TriggerScreenshot),
            VideoCommand::Start => Self::VideoCapture(VideoCaptureMode::StartVideoCapture),
            VideoCommand::Stop => Self::VideoCapture(VideoCaptureMode::EndVideoCapture),
            VideoCommand::Toggle => Self::VideoCapture(VideoCaptureMode::ToggleVideoCapture),
            VideoCommand::ShowTimer => Self::VideoCapture(VideoCaptureMode::ShowVideoTimer),
            VideoCommand::HideTimer => Self::VideoCapture(VideoCaptureMode::HideVideoTimer),
        }
    }
}
