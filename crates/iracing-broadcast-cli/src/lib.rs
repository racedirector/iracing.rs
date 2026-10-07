mod commands;
mod parser;

use anyhow::Result;
use clap::Subcommand;
use iracing_broadcast_sdk::Command as BroadcastCommand;

use crate::commands::{
    CameraCommand, ChatCommand, ForceFeedbackCommand, PitCliCommand, ReplayCommand,
    TelemetryCommand, TextureCommand, VideoCommand,
};

#[derive(Subcommand, Debug, Clone)]
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
        command: ForceFeedbackCommand,
    },
    /// Video and screen capture utilities
    Video {
        #[command(subcommand)]
        command: VideoCommand,
    },
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
            client.send_message(self.into())?;
            Ok(())
        }
    }
}

impl From<Command> for BroadcastCommand {
    /// Build the selected SDK command without sending it.
    ///
    fn from(command: Command) -> Self {
        match command {
            Command::Telemetry { command } => command.into(),
            Command::Ffb { command } => command.into(),
            Command::Video { command } => command.into(),
            Command::Textures { command } => command.into(),
            Command::Chat { command } => command.into(),
            Command::Camera { command } => command.into(),
            Command::Replay { command } => command.into(),
            Command::Pit { command } => command.into(),
        }
    }
}
