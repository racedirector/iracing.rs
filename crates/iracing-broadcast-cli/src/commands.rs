//! Clap declarations and conversion from operator input to typed SDK operations.
//!
//! Exact syntax, defaults, and input constraints live on these declarations and
//! in parser tests. Direct operations map into SDK commands without acquiring
//! simulator resources. Session-time search resolves a session view before
//! dispatch; the injected application owns acquisition and readiness policy.
//! Wire encoding and SDK validation remain in `iracing-broadcast-sdk`.

use crate::session_select::{SessionSelector, SessionTime, resolve_session};
use crate::{BroadcastCommands, ReplaySessions};
use anyhow::Result;
use iracing_broadcast_sdk::{
    CameraState, ChatCommandMode, Command as BroadcastCommand, PitCommand as SdkPitCommand,
    ReplayPositionMode, ReplaySearchMode, ReplayStateMode, TelemetryCommandMode, VideoCaptureMode,
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
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/camera/switch-position.md)."
    )]
    SwitchPosition {
        #[arg(long)]
        position: u16,
        #[arg(long)]
        group: u16,
        #[arg(long, default_value_t = 0)]
        camera: u16,
    },
    /// Switch the camera number
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/camera/switch-number.md)."
    )]
    SwitchNumber {
        #[arg(long)]
        car_number: String,
        #[arg(long, default_value_t = 0)]
        group: u16,
        #[arg(long, default_value_t = 0)]
        camera: u16,
    },
    /// Set the camera state
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/camera/set-state.md)."
    )]
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
/// Direct replay wire operations, converted without live state.
pub enum DirectReplayCommand {
    /// Set replay play speed
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/replay/set-play-speed.md)."
    )]
    SetPlaySpeed {
        #[arg(long)]
        speed: i16,
        #[arg(long, default_value_t = false)]
        slow_motion: bool,
    },
    /// Search the replay for the given mode.
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/replay/search.md)."
    )]
    Search {
        #[arg(value_parser = replay_search_parser())]
        mode: ReplaySearchMode,
    },
    /// Set the replay play head
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/replay/set-play-position.md)."
    )]
    SetPlayPosition {
        #[arg(value_parser = replay_position_parser())]
        mode: ReplayPositionMode,
        #[arg(long)]
        frame: u32,
    },
    /// Erase replay tape
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/replay/erase.md)."
    )]
    Erase,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/replay/normal.md)."
    )]
    Normal,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/replay/slow16.md)."
    )]
    Slow16,
    /// Pause the replay
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/replay/pause.md)."
    )]
    Pause,
}

/// Replay commands: the direct wire operations plus the live-resolved
/// session-time search.
#[derive(clap::Subcommand, Debug, Clone)]
pub enum ReplayCommand {
    /// Jump the replay playhead to a session and time resolved against the
    /// live session metadata. `--session` accepts a numeric session number or
    /// a human selector (`race`, `practice`, `qualify`, `heat`, or a published
    /// session name); `--time` is `SS`, `MM:SS`, or `HH:MM:SS`. Missing or
    /// ambiguous selectors are rejected with the available sessions listed;
    /// nothing is guessed.
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/replay/search-session-time.md)."
    )]
    SearchSessionTime {
        /// Session number or name, resolved against live session metadata
        #[arg(long)]
        session: SessionSelector,
        /// Session time as `SS`, `MM:SS`, or `HH:MM:SS` (e.g. `5:31:20`)
        #[arg(long)]
        time: SessionTime,
    },
    /// Direct replay wire commands
    #[command(flatten)]
    Direct(DirectReplayCommand),
}

impl ReplayCommand {
    /// Execute the replay command through the injected capabilities.
    ///
    /// `SearchSessionTime` resolves its typed selectors against
    /// [`ReplaySessions`] and dispatches the resolved low-level
    /// `ReplaySearchSessionTime` wire command; the direct commands convert and
    /// dispatch unchanged.
    ///
    /// # Errors
    ///
    /// Propagates session acquisition, resolution, initialization, and
    /// dispatch failures.
    pub(crate) fn run(
        self,
        app: &mut (impl BroadcastCommands + ReplaySessions + ?Sized),
    ) -> Result<()> {
        match self {
            Self::SearchSessionTime { session, time } => {
                let resolved = resolve_session(&session, &app.replay_sessions()?)?;
                app.send_broadcast(BroadcastCommand::ReplaySearchSessionTime(
                    resolved.number,
                    time.millis(),
                ))
            }
            Self::Direct(direct) => app.send_broadcast(direct.into()),
        }
    }
}

impl From<DirectReplayCommand> for BroadcastCommand {
    /// Build a replay broadcast command, expanding playback shortcuts.
    ///
    /// `Normal`, `Slow16`, and `Pause` select speed/slow-motion pairs `(1, false)`,
    /// `(16, true)`, and `(0, false)`, respectively.
    fn from(value: DirectReplayCommand) -> Self {
        match value {
            DirectReplayCommand::SetPlaySpeed { speed, slow_motion } => {
                BroadcastCommand::ReplaySetPlaySpeed(speed, slow_motion)
            }
            DirectReplayCommand::Search { mode } => BroadcastCommand::ReplaySearch(mode),
            DirectReplayCommand::SetPlayPosition { mode, frame } => {
                BroadcastCommand::ReplaySetPlayPosition(mode, frame)
            }
            DirectReplayCommand::Erase => {
                BroadcastCommand::ReplaySetState(ReplayStateMode::EraseTape)
            }
            DirectReplayCommand::Normal => BroadcastCommand::ReplaySetPlaySpeed(1, false),
            DirectReplayCommand::Slow16 => BroadcastCommand::ReplaySetPlaySpeed(16, true),
            DirectReplayCommand::Pause => BroadcastCommand::ReplaySetPlaySpeed(0, false),
        }
    }
}

#[derive(clap::Subcommand, Debug, Clone, PartialEq)]
pub enum PitCommand {
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/clear.md)."
    )]
    Clear,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/fuel.md)."
    )]
    Fuel { gallons: u16 },
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/lf.md)."
    )]
    Lf { psi: u16 },
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/rf.md)."
    )]
    Rf { psi: u16 },
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/lr.md)."
    )]
    Lr { psi: u16 },
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/rr.md)."
    )]
    Rr { psi: u16 },
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/clear-tires.md)."
    )]
    ClearTires,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/ws.md)."
    )]
    Ws,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/fr.md)."
    )]
    Fr,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/clear-ws.md)."
    )]
    ClearWs,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/clear-fr.md)."
    )]
    ClearFr,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/pit/clear-fuel.md)."
    )]
    ClearFuel,
}

impl From<PitCommand> for BroadcastCommand {
    /// Build a pit-service broadcast command, passing numeric values through unchanged.
    ///
    /// `Ws` requests a windshield tearoff and `Fr` requests a fast repair;
    /// the corresponding clear commands cancel those requests.
    fn from(value: PitCommand) -> Self {
        match value {
            PitCommand::Clear => BroadcastCommand::Pit(SdkPitCommand::Clear),
            PitCommand::Fuel { gallons } => BroadcastCommand::Pit(SdkPitCommand::Fuel(gallons)),
            PitCommand::Lf { psi } => BroadcastCommand::Pit(SdkPitCommand::LF(psi)),
            PitCommand::Rf { psi } => BroadcastCommand::Pit(SdkPitCommand::RF(psi)),
            PitCommand::Lr { psi } => BroadcastCommand::Pit(SdkPitCommand::LR(psi)),
            PitCommand::Rr { psi } => BroadcastCommand::Pit(SdkPitCommand::RR(psi)),
            PitCommand::ClearTires => BroadcastCommand::Pit(SdkPitCommand::ClearTires),
            PitCommand::Ws => BroadcastCommand::Pit(SdkPitCommand::Tearoff),
            PitCommand::Fr => BroadcastCommand::Pit(SdkPitCommand::FastRepair),
            PitCommand::ClearWs => BroadcastCommand::Pit(SdkPitCommand::ClearTearoff),
            PitCommand::ClearFr => BroadcastCommand::Pit(SdkPitCommand::ClearFastRepair),
            PitCommand::ClearFuel => BroadcastCommand::Pit(SdkPitCommand::ClearFuel),
        }
    }
}

#[derive(clap::Subcommand, Debug, Clone, PartialEq)]
pub enum ChatCommand {
    /// Cancel chat input
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/chat/cancel.md)."
    )]
    Cancel,
    /// Reply to the last received message
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/chat/reply.md)."
    )]
    Reply,
    /// Open chat input
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/chat/begin.md)."
    )]
    Begin,
    /// Send chat macro
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/chat/macro.md)."
    )]
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
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/textures/reload-all.md)."
    )]
    ReloadAll,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/textures/reload-car.md)."
    )]
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
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/telemetry/start.md)."
    )]
    Start,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/telemetry/stop.md)."
    )]
    Stop,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/telemetry/restart.md)."
    )]
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
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/ffb/max-force.md)."
    )]
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
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/video/screenshot.md)."
    )]
    Screenshot,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/video/start.md)."
    )]
    Start,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/video/stop.md)."
    )]
    Stop,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/video/toggle.md)."
    )]
    Toggle,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/video/show-timer.md)."
    )]
    ShowTimer,
    #[cfg_attr(
        doc,
        doc = "Development contract: [document](https://github.com/racedirector/iracing.rs/blob/main/crates/iracing-broadcast-cli/docs/commands/video/hide-timer.md)."
    )]
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
