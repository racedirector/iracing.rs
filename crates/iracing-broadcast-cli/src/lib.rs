mod parser;

use anyhow::Result;
use clap::{Args, Subcommand};
use iracing_broadcast_sdk::{
    CameraState, ChatCommandMode, Command as BroadcastCommand, PitCommand, ReplayPositionMode,
    ReplaySearchMode, ReplayStateMode, TelemetryCommandMode, VideoCaptureMode,
};

use crate::parser::{
    camera_state_parser, replay_position_parser, replay_search_parser, telemetry_command_parser,
    video_command_parser,
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
        #[arg(long, value_parser=telemetry_command_parser())]
        mode: TelemetryCommandMode,
    },
    /// Modify FFB
    Ffb {
        #[arg(long)]
        max_force_nm: f32,
    },
    /// Video and screen capture utilities
    Video {
        #[arg(long, value_parser=video_command_parser())]
        mode: VideoCaptureMode,
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

    /// Build a camera broadcast command, preserving the supplied camera selectors.
    ///
    /// # Errors
    ///
    /// Returns an error if camera state arguments specify both raw bits and flags,
    /// or neither.
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

#[derive(Args, Debug, Clone, PartialEq)]
pub struct CameraStateArgs {
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..=u16::MAX as i64))]
    raw_bits: Option<u32>,
    #[arg(long = "flag", value_parser = camera_state_parser())]
    flags: Vec<CameraState>,
}

impl TryFrom<CameraStateArgs> for CameraState {
    type Error = anyhow::Error;

    /// Build a camera state from raw bits or the union of named flags.
    ///
    /// Raw bits, including unknown flags, are retained unchanged; the CLI parser
    /// limits them to 16 bits. Repeated named flags have no additional effect.
    ///
    /// # Errors
    ///
    /// Returns an error if both raw bits and flags are supplied, or if neither is supplied.
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

#[derive(Subcommand, Debug, Clone, PartialEq)]
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

impl Command {
    /// Send this command through the Windows broadcast client.
    ///
    /// # Errors
    ///
    /// Returns an unsupported-platform error on non-Windows systems. On Windows,
    /// propagates client initialization, camera-state conversion, command encoding,
    /// and Win32 dispatch errors.
    pub fn run(self) -> Result<()> {
        #[cfg(not(windows))]
        {
            Err(anyhow::anyhow!("Broadcast commands only run on windows"))
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

    /// Build the selected SDK command without sending it.
    ///
    /// # Errors
    ///
    /// Returns an error if a camera-state command supplies both raw bits and flags,
    /// or neither. Other command parameters pass through without validation here.
    fn try_from(command: Command) -> Result<Self, Self::Error> {
        match command {
            Command::Telemetry { mode } => Ok(Self::Telemetry(mode)),
            Command::Ffb { max_force_nm } => Ok(Self::ForceFeedback(max_force_nm)),
            Command::Video { mode } => Ok(Self::VideoCapture(mode)),
            Command::Textures { command } => Ok(command.into()),
            Command::Chat { command } => Ok(command.into()),
            Command::Camera { command } => command.try_into(),
            Command::Replay { command } => Ok(command.into()),
            Command::Pit { command } => Ok(command.into()),
        }
    }
}
