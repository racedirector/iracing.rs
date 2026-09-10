use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
#[cfg(windows)]
use iracing_sdk::{
    irsdk::{
        CameraState, ChatCommandMode, PitCommand, ReplayPositionMode, ReplaySearchMode,
        ReplayStateMode, TelemetryCommandMode, VideoCaptureMode,
    },
    windows::{Broadcast, BroadcastCommand},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
enum Command {
    Send {
        #[command(subcommand)]
        command: SendCommand,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
enum SendCommand {
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
enum CameraCommand {
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
struct CameraStateArgs {
    #[arg(long)]
    raw_bits: Option<u32>,
    #[arg(long = "flag", value_enum)]
    flags: Vec<CameraStateFlag>,
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
enum ReplayCommand {
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
enum ChatCommand {
    Cancel,
    Reply,
    Begin,
    Macro {
        #[arg(value_parser = clap::value_parser!(u16).range(1..=15))]
        index: u16,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
enum PitCliCommand {
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
enum TextureCommand {
    ReloadAll,
    ReloadCar { car_idx: u16 },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
enum TelemetryCommand {
    Stop,
    Start,
    Restart,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
enum FfbCliCommand {
    MaxForce { nm: f32 },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
enum VideoCommand {
    Screenshot,
    Start,
    Stop,
    Toggle,
    ShowTimer,
    HideTimer,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ReplaySearchArg {
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

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayPositionArg {
    Begin,
    Current,
    End,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayStateArg {
    EraseTape,
}

fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cli = Cli::parse();

    #[cfg(windows)]
    {
        let client = Broadcast::new().expect("Could not create iRacing broadcast client");

        match cli.command {
            Command::Send { command } => execute_send(&client, command),
        }
    }

    #[cfg(not(windows))]
    {
        let _ = cli;
        tracing::warn!(
            "broadcast-cli is only supported on Windows because iRacing broadcast messaging uses Win32 APIs."
        );
        Err(anyhow::anyhow!(
            "broadcast-cli is only supported on Windows"
        ))
    }
}

#[cfg(windows)]
fn execute_send(client: &Broadcast, command: SendCommand) -> Result<()> {
    let messages = command_to_messages(command)?;
    for message in messages {
        client.send_message(message.clone())?;
        tracing::info!("sent broadcast message: {message:?}");
    }
    Ok(())
}

#[cfg(windows)]
fn command_to_messages(command: SendCommand) -> Result<Vec<BroadcastCommand>> {
    let message = match command {
        SendCommand::Camera { command } => match command {
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
        SendCommand::Replay { command } => match command {
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
        SendCommand::Chat { command } => match command {
            ChatCommand::Cancel => vec![BroadcastCommand::ChatCommand(ChatCommandMode::Cancel)],
            ChatCommand::Reply => vec![BroadcastCommand::ChatCommand(ChatCommandMode::Reply)],
            ChatCommand::Begin => vec![BroadcastCommand::ChatCommand(ChatCommandMode::BeginChat)],
            ChatCommand::Macro { index } => vec![BroadcastCommand::ChatCommandMacro(index)],
        },
        SendCommand::Pit { command } => match command {
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
        SendCommand::Textures { command } => match command {
            TextureCommand::ReloadAll => vec![BroadcastCommand::ReloadAllTextures],
            TextureCommand::ReloadCar { car_idx } => {
                vec![BroadcastCommand::ReloadTextures(car_idx)]
            }
        },
        SendCommand::Telemetry { command } => match command {
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
        SendCommand::Ffb { command } => match command {
            FfbCliCommand::MaxForce { nm } => vec![BroadcastCommand::FFBCommand(nm)],
        },
        SendCommand::Video { command } => match command {
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

#[cfg(windows)]
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

#[cfg(windows)]
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

#[cfg(windows)]
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

#[cfg(windows)]
impl From<ReplayPositionArg> for ReplayPositionMode {
    fn from(mode: ReplayPositionArg) -> Self {
        match mode {
            ReplayPositionArg::Begin => ReplayPositionMode::Begin,
            ReplayPositionArg::Current => ReplayPositionMode::Current,
            ReplayPositionArg::End => ReplayPositionMode::End,
        }
    }
}

#[cfg(windows)]
impl From<ReplayStateArg> for ReplayStateMode {
    fn from(mode: ReplayStateArg) -> Self {
        match mode {
            ReplayStateArg::EraseTape => ReplayStateMode::EraseTape,
        }
    }
}

#[cfg(windows)]
#[link(name = "msvcrt")]
unsafe extern "C" {
    fn _getch() -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chat_macro() {
        let cli = Cli::try_parse_from(["broadcast-cli", "send", "chat", "macro", "15"]).unwrap();
        assert_eq!(
            cli.command,
            Command::Send {
                command: SendCommand::Chat {
                    command: ChatCommand::Macro { index: 15 }
                }
            }
        );
    }

    #[test]
    fn rejects_out_of_range_macro() {
        let err =
            Cli::try_parse_from(["broadcast-cli", "send", "chat", "macro", "16"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("1..=15"));
    }

    #[cfg(windows)]
    #[test]
    fn maps_camera_switch_position_command() {
        let messages = command_to_messages(SendCommand::Camera {
            command: CameraCommand::SwitchPosition {
                position: 1,
                group: 2,
                camera: 3,
            },
        })
        .unwrap();

        assert_eq!(
            messages,
            vec![BroadcastCommand::CameraSwitchPosition(1, 2, 3)]
        );
    }

    #[cfg(windows)]
    #[test]
    fn camera_set_state_requires_input() {
        let err = command_to_messages(SendCommand::Camera {
            command: CameraCommand::SetState(CameraStateArgs {
                raw_bits: None,
                flags: vec![],
            }),
        })
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("requires either --raw-bits or at least one --flag")
        );
    }
}
