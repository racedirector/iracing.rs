use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};
use iracing_broadcast_sdk::{
    CameraState, ChatCommandMode, Command as BroadcastCommand, PitCommand, ReplayPositionMode,
    ReplaySearchMode, ReplayStateMode, TelemetryCommandMode, VideoCaptureMode,
};

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum Command {
    /// Manipulate the camera
    Camera {
        #[command(subcommand)]
        command: CameraCommand,
    },
    /// Modify the replay state
    Replay {
        #[command(subcommand)]
        command: ReplayCommand,
    },
    /// Chat commands
    Chat {
        #[command(subcommand)]
        command: ChatCommand,
    },
    /// Pit service commands
    Pit {
        #[command(subcommand)]
        command: PitCliCommand,
    },
    /// In-sim car textures
    Textures {
        #[command(subcommand)]
        command: TextureCommand,
    },
    /// Modify the disk-telemetry state
    Telemetry {
        #[command(subcommand)]
        command: TelemetryCommand,
    },
    /// Modify FFB
    Ffb {
        #[command(subcommand)]
        command: FfbCliCommand,
    },
    /// Video and screen capture utilities
    Video {
        #[command(subcommand)]
        command: VideoCommand,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
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

impl TryFrom<CameraCommand> for BroadcastCommand {
    type Error = anyhow::Error;

    fn try_from(value: CameraCommand) -> Result<Self, Self::Error> {
        let command = match value {
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
            CameraCommand::SetState(args) => Self::CameraSetState(args.try_into()?),
        };

        Ok(command)
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum CameraStateFlag {
    CamToolActive,
    UiHidden,
    UseAutoShotSelection,
    UseTemporaryEdits,
    UseKeyAcceleration,
    UseKey10xAcceleration,
    UseMouseAimMode,
}

impl From<CameraStateFlag> for CameraState {
    fn from(value: CameraStateFlag) -> Self {
        match value {
            CameraStateFlag::CamToolActive => CameraState::CAMERA_TOOL_ACTIVE,
            CameraStateFlag::UiHidden => CameraState::USER_INTERFACE_HIDDEN,
            CameraStateFlag::UseAutoShotSelection => CameraState::USE_AUTO_SHOT_SELECTION,
            CameraStateFlag::UseTemporaryEdits => CameraState::USE_TEMPORARY_EDITS,
            CameraStateFlag::UseKeyAcceleration => CameraState::USE_KEY_ACCELERATION,
            CameraStateFlag::UseKey10xAcceleration => CameraState::USE_KEY_TEN_TIMES_ACCELERATION,
            CameraStateFlag::UseMouseAimMode => CameraState::USE_MOUSE_AIM_MODE,
        }
    }
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct CameraStateArgs {
    #[arg(long)]
    raw_bits: Option<u32>,
    #[arg(long = "flag", value_enum)]
    flags: Vec<CameraStateFlag>,
}

impl TryFrom<CameraStateArgs> for CameraState {
    type Error = anyhow::Error;

    fn try_from(args: CameraStateArgs) -> std::prelude::v1::Result<Self, Self::Error> {
        if let Some(raw_bits) = args.raw_bits {
            if !args.flags.is_empty() {
                anyhow::bail!("choose either --raw-bits or --flag values, not both");
            }

            return Ok(CameraState::from_bits_retain(raw_bits));
        }

        if args.flags.is_empty() {
            anyhow::bail!("camera set-state requires either --raw-bits or at least one --flag");
        }

        let mut state = CameraState::empty();
        for flag in args.flags {
            state = state.union(flag.into());
        }

        Ok(state)
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaySearchArg {
    /// Set replay play-head to start
    ToStart,
    /// Set replay play-head to end
    ToEnd,
    /// Set replay play-head to previous session
    PrevSession,
    /// Set replay play-head to next session
    NextSession,
    /// Set replay play-head to previous lap
    PrevLap,
    /// Set replay play-head to next lap
    NextLap,
    /// Set replay play-head to previous frame
    PrevFrame,
    /// Set replay play-head to next frame
    NextFrame,
    /// Set replay play-head to previous incident
    PrevIncident,
    /// Set replay play-head to next incident
    NextIncident,
}

impl From<ReplaySearchArg> for ReplaySearchMode {
    fn from(value: ReplaySearchArg) -> Self {
        match value {
            ReplaySearchArg::ToStart => ReplaySearchMode::ToStart,
            ReplaySearchArg::ToEnd => ReplaySearchMode::ToEnd,
            ReplaySearchArg::PrevSession => ReplaySearchMode::PreviousSession,
            ReplaySearchArg::NextSession => ReplaySearchMode::NextSession,
            ReplaySearchArg::PrevLap => ReplaySearchMode::PreviousLap,
            ReplaySearchArg::NextLap => ReplaySearchMode::NextLap,
            ReplaySearchArg::PrevFrame => ReplaySearchMode::PreviousFrame,
            ReplaySearchArg::NextFrame => ReplaySearchMode::NextFrame,
            ReplaySearchArg::PrevIncident => ReplaySearchMode::PreviousIncident,
            ReplaySearchArg::NextIncident => ReplaySearchMode::NextIncident,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayPositionArg {
    Begin,
    Current,
    End,
}

impl From<ReplayPositionArg> for ReplayPositionMode {
    fn from(mode: ReplayPositionArg) -> Self {
        match mode {
            ReplayPositionArg::Begin => ReplayPositionMode::Begin,
            ReplayPositionArg::Current => ReplayPositionMode::Current,
            ReplayPositionArg::End => ReplayPositionMode::End,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayStateArg {
    /// Erase the spooled replay
    EraseTape,
}

impl From<ReplayStateArg> for ReplayStateMode {
    fn from(mode: ReplayStateArg) -> Self {
        match mode {
            ReplayStateArg::EraseTape => ReplayStateMode::EraseTape,
        }
    }
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
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
        #[arg(value_enum)]
        mode: ReplaySearchArg,
    },
    /// Set the replay play head
    SetPlayPosition {
        #[arg(value_enum)]
        mode: ReplayPositionArg,
        #[arg(long)]
        frame: u32,
    },
    /// Set replay state
    SetState {
        #[arg(value_enum, default_value_t = ReplayStateArg::EraseTape)]
        mode: ReplayStateArg,
    },
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
    fn from(value: ReplayCommand) -> Self {
        match value {
            ReplayCommand::SetPlaySpeed { speed, slow_motion } => {
                BroadcastCommand::ReplaySetPlaySpeed(speed, slow_motion)
            }
            ReplayCommand::Search { mode } => BroadcastCommand::ReplaySearch(mode.into()),
            ReplayCommand::SetPlayPosition { mode, frame } => {
                BroadcastCommand::ReplaySetPlayPosition(mode.into(), frame)
            }
            ReplayCommand::SetState { mode } => BroadcastCommand::ReplaySetState(mode.into()),
            ReplayCommand::SearchSessionTime { session, time_ms } => {
                BroadcastCommand::ReplaySearchSessionTime(session, time_ms)
            }
            ReplayCommand::Normal => BroadcastCommand::ReplaySetPlaySpeed(1, false),
            ReplayCommand::Slow16 => BroadcastCommand::ReplaySetPlaySpeed(16, true),
            ReplayCommand::Pause => BroadcastCommand::ReplaySetPlaySpeed(0, false),
        }
    }
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
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

#[derive(Subcommand, Debug, Clone, PartialEq)]
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
    fn from(value: ChatCommand) -> Self {
        match value {
            ChatCommand::Cancel => Self::Chat(ChatCommandMode::Cancel),
            ChatCommand::Reply => Self::Chat(ChatCommandMode::Reply),
            ChatCommand::Begin => Self::Chat(ChatCommandMode::BeginChat),
            ChatCommand::Macro { index } => Self::ChatMacro(index),
        }
    }
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum TextureCommand {
    ReloadAll,
    ReloadCar { car_idx: u16 },
}

impl From<TextureCommand> for BroadcastCommand {
    fn from(value: TextureCommand) -> Self {
        match value {
            TextureCommand::ReloadAll => Self::ReloadAllTextures,
            TextureCommand::ReloadCar { car_idx } => Self::ReloadTextures(car_idx),
        }
    }
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum TelemetryCommand {
    /// Stop disk telemetry
    Stop,
    /// Start disk telemetry
    Start,
    /// Flush and start new recording
    Restart,
}

impl From<TelemetryCommand> for BroadcastCommand {
    fn from(value: TelemetryCommand) -> Self {
        match value {
            TelemetryCommand::Stop => Self::Telemetry(TelemetryCommandMode::Stop),
            TelemetryCommand::Start => Self::Telemetry(TelemetryCommandMode::Start),
            TelemetryCommand::Restart => Self::Telemetry(TelemetryCommandMode::Restart),
        }
    }
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum FfbCliCommand {
    MaxForce { nm: f32 },
}

impl From<FfbCliCommand> for BroadcastCommand {
    fn from(value: FfbCliCommand) -> Self {
        match value {
            FfbCliCommand::MaxForce { nm } => Self::ForceFeedback(nm),
        }
    }
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum VideoCommand {
    /// Capture a screenshot
    Screenshot,
    /// Start recording
    Start,
    /// Stop recording
    Stop,
    /// Toggle recording
    Toggle,
    /// Show video timer
    ShowTimer,
    /// Hide video timer
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

impl Command {
    pub fn run(self) -> Result<()> {
        #[cfg(not(windows))]
        {
            anyhow::anyhow!("Broadcast commands only run on windows")
        }

        #[cfg(windows)]
        {
            let client = iracing_broadcast_sdk::Client::new()?;
            client.send_message(self.try_into()?)?;
            Ok(())
        }
    }
}

impl TryFrom<Command> for BroadcastCommand {
    type Error = anyhow::Error;

    fn try_from(command: Command) -> Result<Self, Self::Error> {
        match command {
            Command::Telemetry { command } => Ok(command.into()),
            Command::Ffb { command } => Ok(command.into()),
            Command::Video { command } => Ok(command.into()),
            Command::Textures { command } => Ok(command.into()),
            Command::Chat { command } => Ok(command.into()),
            Command::Camera { command } => command.try_into(),
            Command::Replay { command } => Ok(command.into()),
            Command::Pit { command } => Ok(command.into()),
        }
    }
}
