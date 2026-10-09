mod commands;
mod parser;

use anyhow::Result;
use clap::Subcommand;
use iracing_broadcast_sdk::Command as BroadcastCommand;

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Manipulate the camera
    Camera {
        #[command(subcommand)]
        command: commands::CameraCommand,
    },
    /// Modify the replay state
    Replay {
        #[command(subcommand)]
        command: commands::ReplayCommand,
    },
    /// Chat commands
    Chat {
        #[command(subcommand)]
        command: commands::ChatCommand,
    },
    /// Pit service commands
    Pit {
        #[command(subcommand)]
        command: commands::PitCommand,
    },
    /// In-sim car textures
    Textures {
        #[command(subcommand)]
        command: commands::TextureCommand,
    },
    /// Modify the disk-telemetry state
    Telemetry {
        #[command(subcommand)]
        command: commands::TelemetryCommand,
    },
    /// Modify FFB
    Ffb {
        #[command(subcommand)]
        command: commands::ForceFeedbackCommand,
    },
    /// Video and screen capture utilities
    Video {
        #[command(subcommand)]
        command: commands::VideoCommand,
    },
}

/// Broadcast dispatch required by the command layer.
///
/// Implementations own transport setup and lifecycle. Fakes can capture commands
/// without simulator infrastructure, on any platform.
pub trait BroadcastCommands {
    /// Dispatch a typed command.
    ///
    /// # Errors
    ///
    /// Propagates implementation-specific initialization and dispatch failures.
    fn send_broadcast(&mut self, command: BroadcastCommand) -> Result<()>;
}

impl Command {
    /// Execute using only the injected broadcast capability.
    ///
    /// # Errors
    ///
    /// Propagates errors from the dependency, including unsupported-platform,
    /// initialization, encoding, and dispatch errors in production applications.
    pub fn run(self, dependencies: &mut (impl BroadcastCommands + ?Sized)) -> Result<()> {
        dependencies.send_broadcast(self.into())
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

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};
    use iracing_broadcast_sdk::{CameraState, ReplayPositionMode, ReplaySearchMode};

    use super::*;

    #[derive(Parser)]
    #[command(name = "iracing-broadcast")]
    struct TestCli {
        #[command(subcommand)]
        command: Command,
    }

    fn parse_command(args: impl IntoIterator<Item = &'static str>) -> Command {
        TestCli::try_parse_from(std::iter::once("iracing-broadcast").chain(args))
            .expect("command should parse")
            .command
    }

    fn command_parse_fails(args: impl IntoIterator<Item = &'static str>) -> bool {
        TestCli::try_parse_from(std::iter::once("iracing-broadcast").chain(args)).is_err()
    }

    #[derive(Default)]
    struct FakeBroadcast(Vec<BroadcastCommand>);

    impl BroadcastCommands for FakeBroadcast {
        fn send_broadcast(&mut self, command: BroadcastCommand) -> Result<()> {
            self.0.push(command);
            Ok(())
        }
    }

    #[test]
    fn parsed_commands_dispatch_through_injected_dependency() -> Result<()> {
        let mut dependency = FakeBroadcast::default();
        parse_command(["replay", "search", "previous-session"]).run(&mut dependency)?;
        parse_command(["camera", "set-state", "--raw-bits", "8"]).run(&mut dependency)?;
        assert_eq!(
            dependency.0,
            [
                BroadcastCommand::ReplaySearch(ReplaySearchMode::PreviousSession),
                BroadcastCommand::CameraSetState(CameraState::from_bits_retain(8)),
            ]
        );
        Ok(())
    }

    #[test]
    fn injected_dispatch_error_is_propagated() {
        struct FailingBroadcast;
        impl BroadcastCommands for FailingBroadcast {
            fn send_broadcast(&mut self, _: BroadcastCommand) -> Result<()> {
                anyhow::bail!("fake dispatch failure")
            }
        }
        let error = parse_command(["replay", "search", "previous-session"])
            .run(&mut FailingBroadcast)
            .unwrap_err();
        assert_eq!(error.to_string(), "fake dispatch failure");
    }

    #[test]
    fn command_definition_is_valid() {
        TestCli::command().debug_assert();
    }

    #[test]
    fn camera_state_accepts_repeated_named_flags() {
        let command = parse_command([
            "camera",
            "set-state",
            "--flag",
            "ui-hidden",
            "--flag",
            "use-mouse-aim",
        ]);

        let expected = CameraState::USER_INTERFACE_HIDDEN.union(CameraState::USE_MOUSE_AIM_MODE);
        assert_eq!(
            BroadcastCommand::from(command),
            BroadcastCommand::CameraSetState(expected)
        );
    }

    #[test]
    fn camera_state_accepts_raw_bits() {
        let command = parse_command(["camera", "set-state", "--raw-bits", "8"]);

        assert_eq!(
            BroadcastCommand::from(command),
            BroadcastCommand::CameraSetState(CameraState::from_bits_retain(8))
        );
    }

    #[test]
    fn camera_state_rejects_raw_bits_with_named_flags() {
        assert!(command_parse_fails([
            "camera",
            "set-state",
            "--raw-bits",
            "8",
            "--flag",
            "ui-hidden",
        ]));
    }

    #[test]
    fn camera_state_requires_raw_bits_or_named_flags() {
        assert!(command_parse_fails(["camera", "set-state"]));
    }

    #[test]
    fn replay_search_parses_domain_mode() {
        let command = parse_command(["replay", "search", "previous-session"]);

        assert_eq!(
            BroadcastCommand::from(command),
            BroadcastCommand::ReplaySearch(ReplaySearchMode::PreviousSession)
        );
    }

    #[test]
    fn replay_position_parses_domain_mode() {
        let command = parse_command(["replay", "set-play-position", "current", "--frame", "123"]);

        assert_eq!(
            BroadcastCommand::from(command),
            BroadcastCommand::ReplaySetPlayPosition(ReplayPositionMode::Current, 123)
        );
    }
}
