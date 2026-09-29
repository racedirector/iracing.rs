use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};
use iracing_sdk::{
    Broadcast, BroadcastCommand, PitCommand,
    irsdk::{
        CameraState, ChatCommandMode, ReplayPositionMode, ReplaySearchMode, ReplayStateMode,
        TelemetryCommandMode, VideoCaptureMode,
    },
};

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub(crate) enum Command {
    Camera {
        #[command(subcommand)]
        command: CameraCommand,
    },
    Replay {
        #[command(subcommand)]
        command: ReplayCommand,
    },
    Chat {
        #[command(subcommand)]
        command: ChatCommand,
    },
    Pit {
        #[command(subcommand)]
        command: PitCliCommand,
    },
    Textures {
        #[command(subcommand)]
        command: TextureCommand,
    },
    Telemetry {
        #[command(subcommand)]
        command: TelemetryCommand,
    },
    Ffb {
        #[command(subcommand)]
        command: FfbCliCommand,
    },
    Video {
        #[command(subcommand)]
        command: VideoCommand,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub(crate) enum CameraCommand {
    SwitchPosition {
        #[arg(long)]
        position: u16,
        #[arg(long)]
        group: u16,
        #[arg(long, default_value_t = 0)]
        camera: u16,
    },
    SwitchNumber {
        #[arg(long)]
        car_number: String,
        #[arg(long)]
        group: u16,
        #[arg(long, default_value_t = 0)]
        camera: u16,
    },
    SetState(CameraStateArgs),
}

#[derive(Args, Debug, Clone, PartialEq)]
pub(crate) struct CameraStateArgs {
    #[arg(long)]
    raw_bits: Option<u32>,
    #[arg(long = "flag", value_enum)]
    flags: Vec<CameraStateFlag>,
}

fn build_camera_state(args: CameraStateArgs) -> Result<CameraState> {
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

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub(crate) enum ReplayCommand {
    SetPlaySpeed {
        #[arg(long)]
        speed: i16,
        #[arg(long, default_value_t = false)]
        slow_motion: bool,
    },
    Search {
        #[arg(value_enum)]
        mode: ReplaySearchArg,
    },
    SetPlayPosition {
        #[arg(value_enum)]
        mode: ReplayPositionArg,
        #[arg(long)]
        frame: u32,
    },
    SetState {
        #[arg(value_enum, default_value_t = ReplayStateArg::EraseTape)]
        mode: ReplayStateArg,
    },
    SearchSessionTime {
        #[arg(long)]
        session: u16,
        #[arg(long)]
        time_ms: u32,
    },
    Normal,
    Slow16,
    Pause,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub(crate) enum ChatCommand {
    Cancel,
    Reply,
    Begin,
    Macro {
        #[arg(value_parser = clap::value_parser!(u16).range(1..=15))]
        index: u16,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub(crate) enum PitCliCommand {
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

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub(crate) enum TextureCommand {
    ReloadAll,
    ReloadCar { car_idx: u16 },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub(crate) enum TelemetryCommand {
    Stop,
    Start,
    Restart,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub(crate) enum FfbCliCommand {
    MaxForce { nm: f32 },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub(crate) enum VideoCommand {
    Screenshot,
    Start,
    Stop,
    Toggle,
    ShowTimer,
    HideTimer,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplaySearchArg {
    ToStart,
    ToEnd,
    PrevSession,
    NextSession,
    PrevLap,
    NextLap,
    PrevFrame,
    NextFrame,
    PrevIncident,
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
pub(crate) enum ReplayPositionArg {
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
pub(crate) enum ReplayStateArg {
    EraseTape,
}

impl From<ReplayStateArg> for ReplayStateMode {
    fn from(mode: ReplayStateArg) -> Self {
        match mode {
            ReplayStateArg::EraseTape => ReplayStateMode::EraseTape,
        }
    }
}

pub(crate) fn handle_command(command: Command) -> Result<()> {
    let client = Broadcast::new()?;

    let messages = command_to_messages(command)?;
    for message in messages {
        client.send_message(message.clone())?;
        tracing::info!("sent broadcast message: {message:?}");
    }

    Ok(())
}

fn command_to_messages(command: Command) -> Result<Vec<BroadcastCommand>> {
    let message = match command {
        Command::Camera { command } => match command {
            CameraCommand::SwitchPosition {
                position,
                group,
                camera,
            } => vec![BroadcastCommand::CameraSwitchPosition(
                position, group, camera,
            )],
            CameraCommand::SwitchNumber {
                car_number,
                group,
                camera,
            } => vec![BroadcastCommand::CameraSwitchNumber(
                car_number, group, camera,
            )],
            CameraCommand::SetState(args) => {
                vec![BroadcastCommand::CameraSetState(build_camera_state(args)?)]
            }
        },
        Command::Replay { command } => match command {
            ReplayCommand::SetPlaySpeed { speed, slow_motion } => {
                vec![BroadcastCommand::ReplaySetPlaySpeed(speed, slow_motion)]
            }
            ReplayCommand::Search { mode } => {
                vec![BroadcastCommand::ReplaySearch(mode.into())]
            }
            ReplayCommand::SetPlayPosition { mode, frame } => {
                vec![BroadcastCommand::ReplaySetPlayPosition(mode.into(), frame)]
            }
            ReplayCommand::SetState { mode } => {
                vec![BroadcastCommand::ReplaySetState(mode.into())]
            }
            ReplayCommand::SearchSessionTime { session, time_ms } => {
                vec![BroadcastCommand::ReplaySearchSessionTime(session, time_ms)]
            }
            ReplayCommand::Normal => vec![BroadcastCommand::ReplaySetPlaySpeed(1, false)],
            ReplayCommand::Slow16 => vec![BroadcastCommand::ReplaySetPlaySpeed(16, true)],
            ReplayCommand::Pause => vec![BroadcastCommand::ReplaySetPlaySpeed(0, false)],
        },
        Command::Chat { command } => match command {
            ChatCommand::Cancel => vec![BroadcastCommand::ChatCommand(ChatCommandMode::Cancel)],
            ChatCommand::Reply => vec![BroadcastCommand::ChatCommand(ChatCommandMode::Reply)],
            ChatCommand::Begin => vec![BroadcastCommand::ChatCommand(ChatCommandMode::BeginChat)],
            ChatCommand::Macro { index } => vec![BroadcastCommand::ChatCommandMacro(index)],
        },
        Command::Pit { command } => match command {
            PitCliCommand::Clear => vec![BroadcastCommand::PitCommand(PitCommand::Clear)],
            PitCliCommand::Fuel { gallons } => {
                vec![BroadcastCommand::PitCommand(PitCommand::Fuel(gallons))]
            }
            PitCliCommand::Lf { psi } => vec![BroadcastCommand::PitCommand(PitCommand::LF(psi))],
            PitCliCommand::Rf { psi } => vec![BroadcastCommand::PitCommand(PitCommand::RF(psi))],
            PitCliCommand::Lr { psi } => vec![BroadcastCommand::PitCommand(PitCommand::LR(psi))],
            PitCliCommand::Rr { psi } => vec![BroadcastCommand::PitCommand(PitCommand::RR(psi))],
            PitCliCommand::ClearTires => {
                vec![BroadcastCommand::PitCommand(PitCommand::ClearTires)]
            }
            PitCliCommand::Ws => vec![BroadcastCommand::PitCommand(PitCommand::Tearoff)],
            PitCliCommand::Fr => vec![BroadcastCommand::PitCommand(PitCommand::FastRepair)],
            PitCliCommand::ClearWs => {
                vec![BroadcastCommand::PitCommand(PitCommand::ClearTearoff)]
            }
            PitCliCommand::ClearFr => {
                vec![BroadcastCommand::PitCommand(PitCommand::ClearFastRepair)]
            }
            PitCliCommand::ClearFuel => {
                vec![BroadcastCommand::PitCommand(PitCommand::ClearFuel)]
            }
        },
        Command::Textures { command } => match command {
            TextureCommand::ReloadAll => vec![BroadcastCommand::ReloadAllTextures],
            TextureCommand::ReloadCar { car_idx } => {
                vec![BroadcastCommand::ReloadTextures(car_idx)]
            }
        },
        Command::Telemetry { command } => match command {
            TelemetryCommand::Stop => {
                vec![BroadcastCommand::TelemetryCommand(
                    TelemetryCommandMode::Stop,
                )]
            }
            TelemetryCommand::Start => {
                vec![BroadcastCommand::TelemetryCommand(
                    TelemetryCommandMode::Start,
                )]
            }
            TelemetryCommand::Restart => {
                vec![BroadcastCommand::TelemetryCommand(
                    TelemetryCommandMode::Restart,
                )]
            }
        },
        Command::Ffb { command } => match command {
            FfbCliCommand::MaxForce { nm } => vec![BroadcastCommand::FFBCommand(nm)],
        },
        Command::Video { command } => match command {
            VideoCommand::Screenshot => {
                vec![BroadcastCommand::VideoCapture(
                    VideoCaptureMode::TriggerScreenshot,
                )]
            }
            VideoCommand::Start => {
                vec![BroadcastCommand::VideoCapture(
                    VideoCaptureMode::StartVideoCapture,
                )]
            }
            VideoCommand::Stop => {
                vec![BroadcastCommand::VideoCapture(
                    VideoCaptureMode::EndVideoCapture,
                )]
            }
            VideoCommand::Toggle => {
                vec![BroadcastCommand::VideoCapture(
                    VideoCaptureMode::ToggleVideoCapture,
                )]
            }
            VideoCommand::ShowTimer => {
                vec![BroadcastCommand::VideoCapture(
                    VideoCaptureMode::ShowVideoTimer,
                )]
            }
            VideoCommand::HideTimer => {
                vec![BroadcastCommand::VideoCapture(
                    VideoCaptureMode::HideVideoTimer,
                )]
            }
        },
    };

    Ok(message)
}
