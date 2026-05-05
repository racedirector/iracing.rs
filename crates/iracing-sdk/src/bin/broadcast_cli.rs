use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
#[cfg(windows)]
use iracing_sdk::windows::{Broadcast, BroadcastCommand, PitCommand};
#[cfg(windows)]
use iracing_sdk::{
    CameraState, ChatCommandMode, ReplayPositionMode, ReplaySearchMode, ReplayStateMode,
    TelemetryCommandMode, VideoCaptureMode,
};
#[cfg(windows)]
use std::io::{self, Write};
use tracing_subscriber::EnvFilter;

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
    Interactive,
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
        position: u8,
        #[arg(long)]
        group: u8,
        #[arg(long, default_value_t = 0)]
        camera: u8,
    },
    SwitchNumber {
        #[arg(long)]
        car_number: String,
        #[arg(long)]
        group: u8,
        #[arg(long, default_value_t = 0)]
        camera: u8,
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
        frame: u16,
    },
    SetState {
        #[arg(value_enum, default_value_t = ReplayStateArg::EraseTape)]
        mode: ReplayStateArg,
    },
    SearchSessionTime {
        #[arg(long)]
        session: u8,
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
        #[arg(value_parser = clap::value_parser!(u8).range(0..=14))]
        index: u8,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
enum PitCliCommand {
    Clear,
    Fuel { gallons: u8 },
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
    ReloadCar { car_idx: u8 },
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
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run()
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    #[cfg(windows)]
    {
        run_windows(cli)
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
fn run_windows(cli: Cli) -> Result<()> {
    let client = Broadcast::new().expect("Could not create iRacing broadcast client");

    match cli.command {
        Command::Send { command } => execute_send(&client, command),
        Command::Interactive => run_interactive(&client),
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
                vec![BroadcastCommand::ReplaySearch(replay_search_mode(mode))]
            }
            ReplayCommand::SetPlayPosition { mode, frame } => {
                vec![BroadcastCommand::ReplaySetPlayPosition(
                    replay_position_mode(mode),
                    frame,
                )]
            }
            ReplayCommand::SetState { mode } => {
                vec![BroadcastCommand::ReplaySetState(replay_state_mode(mode))]
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
                    VideoCaptureMode::TriggerScreenShot,
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
        state = state.union(camera_state_flag(flag));
    }

    Ok(state)
}

#[cfg(windows)]
fn camera_state_flag(flag: CameraStateFlag) -> CameraState {
    match flag {
        CameraStateFlag::CamToolActive => CameraState::CAM_TOOL_ACTIVE,
        CameraStateFlag::UiHidden => CameraState::UI_HIDDEN,
        CameraStateFlag::UseAutoShotSelection => CameraState::USE_AUTO_SHOT_SELECTION,
        CameraStateFlag::UseTemporaryEdits => CameraState::USE_TEMPORARY_EDITS,
        CameraStateFlag::UseKeyAcceleration => CameraState::USE_KEY_ACCELERATION,
        CameraStateFlag::UseKey10xAcceleration => CameraState::USE_KEY_10X_ACCELERATION,
        CameraStateFlag::UseMouseAimMode => CameraState::USE_MOUSE_AIM_MODE,
    }
}

#[cfg(windows)]
fn replay_search_mode(mode: ReplaySearchArg) -> ReplaySearchMode {
    match mode {
        ReplaySearchArg::ToStart => ReplaySearchMode::ToStart,
        ReplaySearchArg::ToEnd => ReplaySearchMode::ToEnd,
        ReplaySearchArg::PrevSession => ReplaySearchMode::PrevSession,
        ReplaySearchArg::NextSession => ReplaySearchMode::NextSession,
        ReplaySearchArg::PrevLap => ReplaySearchMode::PrevLap,
        ReplaySearchArg::NextLap => ReplaySearchMode::NextLap,
        ReplaySearchArg::PrevFrame => ReplaySearchMode::PrevFrame,
        ReplaySearchArg::NextFrame => ReplaySearchMode::NextFrame,
        ReplaySearchArg::PrevIncident => ReplaySearchMode::PrevIncident,
        ReplaySearchArg::NextIncident => ReplaySearchMode::NextIncident,
    }
}

#[cfg(windows)]
fn replay_position_mode(mode: ReplayPositionArg) -> ReplayPositionMode {
    match mode {
        ReplayPositionArg::Begin => ReplayPositionMode::Begin,
        ReplayPositionArg::Current => ReplayPositionMode::Current,
        ReplayPositionArg::End => ReplayPositionMode::End,
    }
}

#[cfg(windows)]
fn replay_state_mode(mode: ReplayStateArg) -> ReplayStateMode {
    match mode {
        ReplayStateArg::EraseTape => ReplayStateMode::EraseTape,
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct InteractiveState {
    play_speed: i16,
    slow_motion: bool,
    replay_search_idx: usize,
    replay_position_idx: usize,
    replay_frame: u16,
    camera_state_enabled: bool,
}

#[cfg(windows)]
impl Default for InteractiveState {
    fn default() -> Self {
        Self {
            play_speed: 16,
            slow_motion: false,
            replay_search_idx: 0,
            replay_position_idx: 0,
            replay_frame: 600,
            camera_state_enabled: false,
        }
    }
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq)]
enum InteractiveAction {
    Send(Vec<BroadcastCommand>),
    Noop,
    Exit,
}

#[cfg(windows)]
trait PromptInput {
    fn prompt_line(&mut self, label: &str) -> Result<String>;
}

#[cfg(windows)]
struct StdinPrompter;

#[cfg(windows)]
impl PromptInput for StdinPrompter {
    fn prompt_line(&mut self, label: &str) -> Result<String> {
        prompt_line(label)
    }
}

#[cfg(windows)]
fn prompt_line(label: &str) -> Result<String> {
    print!("{label}: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

#[cfg(windows)]
fn is_cancel(s: &str) -> bool {
    s.is_empty() || s.eq_ignore_ascii_case("q")
}

#[cfg(windows)]
fn parse_u8_input(input: &str, min: u8, max: u8) -> Result<Option<u8>> {
    if is_cancel(input) {
        return Ok(None);
    }
    let value = input.parse::<u8>()?;
    if value < min || value > max {
        anyhow::bail!("value must be in range {min}..={max}");
    }
    Ok(Some(value))
}

#[cfg(windows)]
fn parse_u16_input(input: &str, min: u16, max: u16) -> Result<Option<u16>> {
    if is_cancel(input) {
        return Ok(None);
    }
    let value = input.parse::<u16>()?;
    if value < min || value > max {
        anyhow::bail!("value must be in range {min}..={max}");
    }
    Ok(Some(value))
}

#[cfg(windows)]
fn parse_u32_input(input: &str, min: u32, max: u32) -> Result<Option<u32>> {
    if is_cancel(input) {
        return Ok(None);
    }
    let value = input.parse::<u32>()?;
    if value < min || value > max {
        anyhow::bail!("value must be in range {min}..={max}");
    }
    Ok(Some(value))
}

#[cfg(windows)]
fn parse_f32_input(input: &str, min: Option<f32>, max: Option<f32>) -> Result<Option<f32>> {
    if is_cancel(input) {
        return Ok(None);
    }
    let value = input.parse::<f32>()?;
    if let Some(min) = min
        && value < min
    {
        anyhow::bail!("value must be >= {min}");
    }
    if let Some(max) = max
        && value > max
    {
        anyhow::bail!("value must be <= {max}");
    }

    Ok(Some(value))
}

#[cfg(windows)]
fn prompt_u8<P: PromptInput>(
    prompter: &mut P,
    label: &str,
    min: u8,
    max: u8,
) -> Result<Option<u8>> {
    loop {
        let input = prompter.prompt_line(label)?;
        match parse_u8_input(&input, min, max) {
            Ok(value) => return Ok(value),
            Err(err) => {
                tracing::warn!("{err}. Enter a value in {min}..={max}, or press Enter/q to cancel.")
            }
        }
    }
}

#[cfg(windows)]
fn prompt_u16<P: PromptInput>(
    prompter: &mut P,
    label: &str,
    min: u16,
    max: u16,
) -> Result<Option<u16>> {
    loop {
        let input = prompter.prompt_line(label)?;
        match parse_u16_input(&input, min, max) {
            Ok(value) => return Ok(value),
            Err(err) => {
                tracing::warn!("{err}. Enter a value in {min}..={max}, or press Enter/q to cancel.")
            }
        }
    }
}

#[cfg(windows)]
fn prompt_u32<P: PromptInput>(
    prompter: &mut P,
    label: &str,
    min: u32,
    max: u32,
) -> Result<Option<u32>> {
    loop {
        let input = prompter.prompt_line(label)?;
        match parse_u32_input(&input, min, max) {
            Ok(value) => return Ok(value),
            Err(err) => {
                tracing::warn!("{err}. Enter a value in {min}..={max}, or press Enter/q to cancel.")
            }
        }
    }
}

#[cfg(windows)]
fn prompt_f32<P: PromptInput>(
    prompter: &mut P,
    label: &str,
    min: Option<f32>,
    max: Option<f32>,
) -> Result<Option<f32>> {
    loop {
        let input = prompter.prompt_line(label)?;
        match parse_f32_input(&input, min, max) {
            Ok(value) => return Ok(value),
            Err(err) => {
                tracing::warn!("{err}. Enter a numeric value, or press Enter/q to cancel.");
            }
        }
    }
}

#[cfg(windows)]
fn prompt_text<P: PromptInput>(prompter: &mut P, label: &str) -> Result<Option<String>> {
    loop {
        let input = prompter.prompt_line(label)?;
        if is_cancel(&input) {
            return Ok(None);
        }
        if input.trim().is_empty() {
            tracing::warn!("value cannot be empty. Press Enter/q to cancel.");
            continue;
        }
        return Ok(Some(input));
    }
}

#[cfg(windows)]
fn interactive_action_for_key<P: PromptInput>(
    key: char,
    state: &mut InteractiveState,
    prompter: &mut P,
) -> Result<InteractiveAction> {
    const SEARCH_MODES: [ReplaySearchMode; 10] = [
        ReplaySearchMode::ToStart,
        ReplaySearchMode::ToEnd,
        ReplaySearchMode::PrevSession,
        ReplaySearchMode::NextSession,
        ReplaySearchMode::PrevLap,
        ReplaySearchMode::NextLap,
        ReplaySearchMode::PrevFrame,
        ReplaySearchMode::NextFrame,
        ReplaySearchMode::PrevIncident,
        ReplaySearchMode::NextIncident,
    ];

    const POSITION_MODES: [ReplayPositionMode; 3] = [
        ReplayPositionMode::Begin,
        ReplayPositionMode::Current,
        ReplayPositionMode::End,
    ];

    let action = match key {
        'a' => InteractiveAction::Send(vec![BroadcastCommand::CameraSwitchPosition(1, 1, 0)]),
        'b' => {
            let Some(car_number) = prompt_text(prompter, "Car number (or Enter/q to cancel)")?
            else {
                return Ok(InteractiveAction::Noop);
            };
            let Some(group) = prompt_u8(prompter, "Camera group", 0, u8::MAX)? else {
                return Ok(InteractiveAction::Noop);
            };
            let Some(camera) = prompt_u8(prompter, "Camera index", 0, u8::MAX)? else {
                return Ok(InteractiveAction::Noop);
            };
            InteractiveAction::Send(vec![BroadcastCommand::CameraSwitchNumber(
                car_number, group, camera,
            )])
        }
        'c' => InteractiveAction::Send(vec![BroadcastCommand::CameraSwitchPosition(0, 3, 0)]),
        'd' => {
            let action = InteractiveAction::Send(vec![BroadcastCommand::ReplaySetPlaySpeed(
                state.play_speed,
                state.slow_motion,
            )]);
            state.play_speed -= 1;
            if state.play_speed < -16 {
                state.play_speed = 16;
                state.slow_motion = !state.slow_motion;
            }
            action
        }
        'e' => {
            let mode = SEARCH_MODES[state.replay_search_idx];
            state.replay_search_idx = (state.replay_search_idx + 1) % SEARCH_MODES.len();
            InteractiveAction::Send(vec![BroadcastCommand::ReplaySearch(mode)])
        }
        'f' => {
            let mode = POSITION_MODES[state.replay_position_idx];
            state.replay_position_idx = (state.replay_position_idx + 1) % POSITION_MODES.len();
            InteractiveAction::Send(vec![BroadcastCommand::ReplaySetPlayPosition(
                mode,
                state.replay_frame,
            )])
        }
        'g' => {
            let state_bits = if state.camera_state_enabled {
                CameraState::empty()
            } else {
                CameraState::CAM_TOOL_ACTIVE
                    .union(CameraState::UI_HIDDEN)
                    .union(CameraState::USE_AUTO_SHOT_SELECTION)
                    .union(CameraState::USE_TEMPORARY_EDITS)
                    .union(CameraState::USE_KEY_ACCELERATION)
                    .union(CameraState::USE_KEY_10X_ACCELERATION)
                    .union(CameraState::USE_MOUSE_AIM_MODE)
            };
            state.camera_state_enabled = !state.camera_state_enabled;
            InteractiveAction::Send(vec![BroadcastCommand::CameraSetState(state_bits)])
        }
        'h' => InteractiveAction::Send(vec![BroadcastCommand::ReplaySetState(
            ReplayStateMode::EraseTape,
        )]),
        'i' => {
            InteractiveAction::Send(vec![BroadcastCommand::ChatCommand(ChatCommandMode::Cancel)])
        }
        'j' => InteractiveAction::Send(vec![BroadcastCommand::ChatCommand(ChatCommandMode::Reply)]),
        'k' => InteractiveAction::Send(vec![BroadcastCommand::ChatCommand(
            ChatCommandMode::BeginChat,
        )]),
        'l' => {
            let Some(macro_id) = prompt_u8(prompter, "Chat macro index", 0, 14)? else {
                return Ok(InteractiveAction::Noop);
            };
            InteractiveAction::Send(vec![BroadcastCommand::ChatCommandMacro(macro_id)])
        }
        'm' => InteractiveAction::Send(vec![BroadcastCommand::PitCommand(PitCommand::Clear)]),
        'n' => {
            let Some(gallons) = prompt_u8(prompter, "Fuel to add (gallons)", 0, u8::MAX)? else {
                return Ok(InteractiveAction::Noop);
            };
            InteractiveAction::Send(vec![BroadcastCommand::PitCommand(PitCommand::Fuel(
                gallons,
            ))])
        }
        'o' => {
            let Some(lf) = prompt_u16(prompter, "LF pressure", 0, u16::MAX)? else {
                return Ok(InteractiveAction::Noop);
            };
            let Some(rf) = prompt_u16(prompter, "RF pressure", 0, u16::MAX)? else {
                return Ok(InteractiveAction::Noop);
            };
            let Some(lr) = prompt_u16(prompter, "LR pressure", 0, u16::MAX)? else {
                return Ok(InteractiveAction::Noop);
            };
            let Some(rr) = prompt_u16(prompter, "RR pressure", 0, u16::MAX)? else {
                return Ok(InteractiveAction::Noop);
            };
            InteractiveAction::Send(vec![
                BroadcastCommand::PitCommand(PitCommand::LF(lf)),
                BroadcastCommand::PitCommand(PitCommand::RF(rf)),
                BroadcastCommand::PitCommand(PitCommand::LR(lr)),
                BroadcastCommand::PitCommand(PitCommand::RR(rr)),
            ])
        }
        'p' => InteractiveAction::Send(vec![BroadcastCommand::PitCommand(PitCommand::Tearoff)]),
        'q' => InteractiveAction::Send(vec![BroadcastCommand::PitCommand(PitCommand::ClearTires)]),
        'r' => InteractiveAction::Send(vec![BroadcastCommand::ReloadAllTextures]),
        's' => {
            let Some(car_idx) = prompt_u8(prompter, "Car index", 0, u8::MAX)? else {
                return Ok(InteractiveAction::Noop);
            };
            InteractiveAction::Send(vec![BroadcastCommand::ReloadTextures(car_idx)])
        }
        't' => InteractiveAction::Send(vec![BroadcastCommand::ReplaySetPlaySpeed(1, false)]),
        'u' => InteractiveAction::Send(vec![BroadcastCommand::ReplaySetPlaySpeed(16, true)]),
        'v' => InteractiveAction::Send(vec![BroadcastCommand::ReplaySetPlaySpeed(0, false)]),
        'w' => InteractiveAction::Send(vec![BroadcastCommand::TelemetryCommand(
            TelemetryCommandMode::Stop,
        )]),
        'x' => InteractiveAction::Send(vec![BroadcastCommand::TelemetryCommand(
            TelemetryCommandMode::Start,
        )]),
        'y' => InteractiveAction::Send(vec![BroadcastCommand::TelemetryCommand(
            TelemetryCommandMode::Restart,
        )]),
        'z' => {
            let Some(force_nm) = prompt_f32(
                prompter,
                "FFB max force Nm (negative for user mode)",
                None,
                None,
            )?
            else {
                return Ok(InteractiveAction::Noop);
            };
            InteractiveAction::Send(vec![BroadcastCommand::FFBCommand(force_nm)])
        }
        'A' => {
            let Some(session) = prompt_u8(prompter, "Session number", 0, u8::MAX)? else {
                return Ok(InteractiveAction::Noop);
            };
            let Some(time_ms) = prompt_u32(prompter, "Session time (ms)", 0, u32::MAX)? else {
                return Ok(InteractiveAction::Noop);
            };
            InteractiveAction::Send(vec![BroadcastCommand::ReplaySearchSessionTime(
                session, time_ms,
            )])
        }
        'B' => InteractiveAction::Send(vec![BroadcastCommand::PitCommand(PitCommand::FastRepair)]),
        'C' => {
            InteractiveAction::Send(vec![BroadcastCommand::PitCommand(PitCommand::ClearTearoff)])
        }
        'D' => InteractiveAction::Send(vec![BroadcastCommand::PitCommand(
            PitCommand::ClearFastRepair,
        )]),
        'E' => InteractiveAction::Send(vec![BroadcastCommand::PitCommand(PitCommand::ClearFuel)]),
        'F' => InteractiveAction::Send(vec![BroadcastCommand::VideoCapture(
            VideoCaptureMode::TriggerScreenShot,
        )]),
        'G' => InteractiveAction::Send(vec![BroadcastCommand::VideoCapture(
            VideoCaptureMode::StartVideoCapture,
        )]),
        'H' => InteractiveAction::Send(vec![BroadcastCommand::VideoCapture(
            VideoCaptureMode::EndVideoCapture,
        )]),
        'I' => InteractiveAction::Send(vec![BroadcastCommand::VideoCapture(
            VideoCaptureMode::ToggleVideoCapture,
        )]),
        'J' => InteractiveAction::Send(vec![BroadcastCommand::VideoCapture(
            VideoCaptureMode::ShowVideoTimer,
        )]),
        'K' => InteractiveAction::Send(vec![BroadcastCommand::VideoCapture(
            VideoCaptureMode::HideVideoTimer,
        )]),
        _ => InteractiveAction::Exit,
    };

    Ok(action)
}

#[cfg(windows)]
fn run_interactive(client: &Broadcast) -> Result<()> {
    println!("iRacing remote control demo:");
    println!(" - press 'a' to switch cameras by position.");
    println!(" - press 'b' to switch cameras by driver # (prompts for input).");
    println!(" - press 'c' to switch cameras but not driver.");
    println!(" - press 'd' to cycle replay playback speed.");
    println!(" - press 'e' to cycle replay search mode.");
    println!(" - press 'f' to cycle replay set playback position.");
    println!(" - press 'g' to cycle camera state.");
    println!(" - press 'h' to clear the replay tape.");
    println!();
    println!(" - press 'i' to clear the chat window.");
    println!(" - press 'j' to reply to a private chat.");
    println!(" - press 'k' to activate the chat window.");
    println!(" - press 'l' to activate a chat macro (prompts for input).");
    println!();
    println!(" - press 'm' to clear all pitstop commands.");
    println!(" - press 'n' to add fuel (prompts for input).");
    println!(" - press 'o' to change tires (prompts for input).");
    println!(" - press 'p' to clean windows.");
    println!(" - press 'q' to clear tire pitstop commands.");
    println!();
    println!(" - press 'r' to reload custom car textures for all cars.");
    println!(
        " - press 's' to reload custom car textures for a specific carIdx (prompts for input)."
    );
    println!();
    println!(" - press 't' to play at normal speed.");
    println!(" - press 'u' to play at 1/16th speed.");
    println!(" - press 'v' to pause the replay.");
    println!();
    println!(" - press 'w' to stop recording telemetry to disk.");
    println!(" - press 'x' to start recording telemetry to disk.");
    println!(" - press 'y' to save out old telemetry and start new one.");
    println!(" - press 'z' to set FFB max force (prompts for input).");
    println!(" - press 'A' to go to a session/time (prompts for input).");
    println!();
    println!(" - press 'B' to request a fast repair.");
    println!(" - press 'C' to uncheck clear windshield.");
    println!(" - press 'D' to uncheck a fast repair.");
    println!(" - press 'E' to uncheck add fuel.");
    println!();
    println!(" - press 'F' to trigger screen shot.");
    println!(" - press 'G' to start video capture.");
    println!(" - press 'H' to stop video capture.");
    println!(" - press 'I' to toggle video capture.");
    println!(" - press 'J' to show video timer.");
    println!(" - press 'K' to hide video timer.");
    println!();
    println!(" prompt tips: press Enter or type 'q' to cancel a prompted action.");
    println!(" press any other key to exit\n");

    let mut state = InteractiveState::default();
    let mut prompter = StdinPrompter;

    loop {
        let c = getch() as u8 as char;
        match interactive_action_for_key(c, &mut state, &mut prompter)? {
            InteractiveAction::Send(messages) => {
                for message in messages {
                    client.send_message(message.clone())?;
                    tracing::info!("sent broadcast message from interactive mode: {message:?}");
                }
            }
            InteractiveAction::Noop => tracing::info!("interactive action canceled"),
            InteractiveAction::Exit => break,
        }
    }

    Ok(())
}

#[cfg(windows)]
fn getch() -> i32 {
    unsafe { _getch() }
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
        let cli = Cli::try_parse_from(["broadcast-cli", "send", "chat", "macro", "14"]).unwrap();
        assert_eq!(
            cli.command,
            Command::Send {
                command: SendCommand::Chat {
                    command: ChatCommand::Macro { index: 14 }
                }
            }
        );
    }

    #[test]
    fn rejects_out_of_range_macro() {
        let err =
            Cli::try_parse_from(["broadcast-cli", "send", "chat", "macro", "15"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("0..=14"));
    }

    #[cfg(windows)]
    struct MockPrompter {
        inputs: std::collections::VecDeque<String>,
    }

    #[cfg(windows)]
    impl MockPrompter {
        fn new(inputs: &[&str]) -> Self {
            Self {
                inputs: inputs.iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    #[cfg(windows)]
    impl PromptInput for MockPrompter {
        fn prompt_line(&mut self, _label: &str) -> Result<String> {
            Ok(self.inputs.pop_front().unwrap_or_default())
        }
    }

    #[cfg(windows)]
    #[test]
    fn parse_prompt_u8_valid_and_cancel() {
        assert_eq!(parse_u8_input("14", 0, 20).unwrap(), Some(14));
        assert_eq!(parse_u8_input("", 0, 20).unwrap(), None);
        assert_eq!(parse_u8_input("q", 0, 20).unwrap(), None);
    }

    #[cfg(windows)]
    #[test]
    fn parse_prompt_u8_rejects_out_of_range() {
        let err = parse_u8_input("15", 0, 14).unwrap_err();
        assert!(err.to_string().contains("0..=14"));
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
    fn interactive_key_map_a_and_unknown() {
        let mut state = InteractiveState::default();
        let mut prompter = MockPrompter::new(&[]);

        assert_eq!(
            interactive_action_for_key('a', &mut state, &mut prompter).unwrap(),
            InteractiveAction::Send(vec![BroadcastCommand::CameraSwitchPosition(1, 1, 0)])
        );
        assert_eq!(
            interactive_action_for_key('?', &mut state, &mut prompter).unwrap(),
            InteractiveAction::Exit
        );
    }

    #[cfg(windows)]
    #[test]
    fn interactive_key_d_cycles_speed() {
        let mut state = InteractiveState::default();
        let mut prompter = MockPrompter::new(&[]);
        let action = interactive_action_for_key('d', &mut state, &mut prompter).unwrap();
        assert_eq!(
            action,
            InteractiveAction::Send(vec![BroadcastCommand::ReplaySetPlaySpeed(16, false)])
        );
        assert_eq!(state.play_speed, 15);
        assert!(!state.slow_motion);
    }

    #[cfg(windows)]
    #[test]
    fn interactive_s_prompts_for_car_idx() {
        let mut state = InteractiveState::default();
        let mut prompter = MockPrompter::new(&["7"]);
        let action = interactive_action_for_key('s', &mut state, &mut prompter).unwrap();
        assert_eq!(
            action,
            InteractiveAction::Send(vec![BroadcastCommand::ReloadTextures(7)])
        );
    }

    #[cfg(windows)]
    #[test]
    fn interactive_s_cancel_is_noop() {
        let mut state = InteractiveState::default();
        let mut prompter = MockPrompter::new(&[""]);
        let action = interactive_action_for_key('s', &mut state, &mut prompter).unwrap();
        assert_eq!(action, InteractiveAction::Noop);
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
