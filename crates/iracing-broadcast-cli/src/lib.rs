mod commands;
mod parser;
pub mod session_select;

use anyhow::Result;
use clap::Subcommand;
use iracing_broadcast_sdk::Command as BroadcastCommand;
pub use session_select::{ReplaySession, SessionSelector, SessionTime};

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

/// Sessions available for replay lookup.
///
/// Implementations own live session acquisition and readiness policy, including
/// any bounded wait for session metadata, and map the result into the
/// command-owned [`ReplaySession`] view. Acquisition failures return an error;
/// an empty list means the metadata published no sessions, not that the
/// simulator is absent. Fakes can script sessions without simulator
/// infrastructure, on any platform.
pub trait ReplaySessions {
    /// Return the sessions currently available for replay lookup.
    ///
    /// # Errors
    ///
    /// Propagates implementation-specific initialization, acquisition, and
    /// readiness failures.
    fn replay_sessions(&mut self) -> Result<Vec<ReplaySession>>;
}

/// Shared transport adapter for CLI application composition.
///
/// Each application owns its own instance and chooses when to initialize it.
/// Commands consume [`BroadcastCommands`] rather than constructing this adapter.
#[derive(Debug)]
pub struct BroadcastClient {
    #[cfg(windows)]
    client: iracing_broadcast_sdk::Client,
}

impl BroadcastClient {
    /// Construct the concrete transport adapter.
    ///
    /// CLI applications invoke this constructor lazily and retain the adapter.
    /// On Windows this registers the SDK broadcast message; it does not wait for
    /// a running simulator.
    ///
    /// # Errors
    ///
    /// Returns an unsupported-platform error outside Windows, or propagates SDK
    /// client initialization errors on Windows.
    pub fn new() -> Result<Self> {
        #[cfg(windows)]
        {
            Ok(Self {
                client: iracing_broadcast_sdk::Client::new()?,
            })
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("Broadcast commands only run on Windows")
        }
    }
}

impl BroadcastCommands for BroadcastClient {
    /// Send the command through the retained Windows broadcast client.
    ///
    /// # Errors
    ///
    /// Propagates command encoding and Win32 dispatch errors on Windows.
    /// Returns an unsupported-platform error elsewhere.
    fn send_broadcast(&mut self, command: BroadcastCommand) -> Result<()> {
        #[cfg(windows)]
        {
            Ok(self.client.send_message(command)?)
        }
        #[cfg(not(windows))]
        {
            let _ = command;
            anyhow::bail!("Broadcast commands only run on Windows")
        }
    }
}

impl Command {
    /// Execute using the injected capabilities.
    ///
    /// Every command dispatches through [`BroadcastCommands`]; the replay
    /// group additionally resolves session selectors against
    /// [`ReplaySessions`] before its wire command is built.
    ///
    /// # Errors
    ///
    /// Propagates errors from the dependencies, including unsupported-platform,
    /// initialization, session acquisition and resolution, encoding, and
    /// dispatch errors in production applications.
    pub fn run(self, app: &mut (impl BroadcastCommands + ReplaySessions + ?Sized)) -> Result<()> {
        match self {
            Command::Camera { command } => app.send_broadcast(command.into()),
            Command::Replay { command } => command.run(app),
            Command::Chat { command } => app.send_broadcast(command.into()),
            Command::Pit { command } => app.send_broadcast(command.into()),
            Command::Textures { command } => app.send_broadcast(command.into()),
            Command::Telemetry { command } => app.send_broadcast(command.into()),
            Command::Ffb { command } => app.send_broadcast(command.into()),
            Command::Video { command } => app.send_broadcast(command.into()),
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

    fn wire(command: Command) -> BroadcastCommand {
        match command {
            Command::Camera { command } => command.into(),
            Command::Replay { .. } => {
                unreachable!("replay dispatch is covered by the run-path tests")
            }
            Command::Chat { command } => command.into(),
            Command::Pit { command } => command.into(),
            Command::Textures { command } => command.into(),
            Command::Telemetry { command } => command.into(),
            Command::Ffb { command } => command.into(),
            Command::Video { command } => command.into(),
        }
    }

    fn session(number: u16, kind: &str, name: Option<&str>) -> ReplaySession {
        ReplaySession {
            number,
            session_type: kind.to_string(),
            name: name.map(str::to_string),
        }
    }

    fn weekend_sessions() -> Vec<ReplaySession> {
        vec![
            session(0, "Practice", Some("Practice")),
            session(1, "Qualify", Some("Qualify")),
            session(2, "Race", Some("Race")),
        ]
    }

    #[derive(Default)]
    struct FakeBroadcast {
        sent: Vec<BroadcastCommand>,
        sessions: Option<Vec<ReplaySession>>,
    }

    impl BroadcastCommands for FakeBroadcast {
        fn send_broadcast(&mut self, command: BroadcastCommand) -> Result<()> {
            self.sent.push(command);
            Ok(())
        }
    }

    impl ReplaySessions for FakeBroadcast {
        fn replay_sessions(&mut self) -> Result<Vec<ReplaySession>> {
            self.sessions
                .clone()
                .ok_or_else(|| anyhow::anyhow!("fake has no session metadata"))
        }
    }

    #[test]
    fn parsed_commands_dispatch_through_injected_dependency() -> Result<()> {
        let mut dependency = FakeBroadcast::default();
        parse_command(["replay", "search", "previous-session"]).run(&mut dependency)?;
        parse_command(["camera", "set-state", "--raw-bits", "8"]).run(&mut dependency)?;
        assert_eq!(
            dependency.sent,
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
        impl ReplaySessions for FailingBroadcast {
            fn replay_sessions(&mut self) -> Result<Vec<ReplaySession>> {
                Ok(Vec::new())
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
        assert_eq!(wire(command), BroadcastCommand::CameraSetState(expected));
    }

    #[test]
    fn camera_state_accepts_raw_bits() {
        let command = parse_command(["camera", "set-state", "--raw-bits", "8"]);

        assert_eq!(
            wire(command),
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
        let Command::Replay {
            command: commands::ReplayCommand::Direct(direct),
        } = parse_command(["replay", "search", "previous-session"])
        else {
            panic!("expected a direct replay search");
        };

        assert_eq!(
            BroadcastCommand::from(direct),
            BroadcastCommand::ReplaySearch(ReplaySearchMode::PreviousSession)
        );
    }

    #[test]
    fn replay_position_parses_domain_mode() {
        let Command::Replay {
            command: commands::ReplayCommand::Direct(direct),
        } = parse_command(["replay", "set-play-position", "current", "--frame", "123"])
        else {
            panic!("expected a direct replay position command");
        };

        assert_eq!(
            BroadcastCommand::from(direct),
            BroadcastCommand::ReplaySetPlayPosition(ReplayPositionMode::Current, 123)
        );
    }

    #[test]
    fn replay_group_keeps_direct_subcommands_alongside_resolved_search() {
        assert!(matches!(
            parse_command(["replay", "pause"]),
            Command::Replay {
                command: commands::ReplayCommand::Direct(commands::DirectReplayCommand::Pause),
            }
        ));
        assert!(matches!(
            parse_command(["replay", "erase"]),
            Command::Replay {
                command: commands::ReplayCommand::Direct(commands::DirectReplayCommand::Erase),
            }
        ));
    }

    #[test]
    fn search_session_time_parses_typed_selectors() {
        let Command::Replay {
            command: commands::ReplayCommand::SearchSessionTime { session, time },
        } = parse_command([
            "replay",
            "search-session-time",
            "--session",
            "race",
            "--time",
            "7:50",
        ])
        else {
            panic!("expected a resolved session-time search");
        };
        assert_eq!(session.key(), Some("race"));
        assert!(!session.is_number());
        assert_eq!(time.millis(), 470_000);
    }

    #[test]
    fn search_session_time_rejects_bad_selector_syntax_at_parse_time() {
        assert!(command_parse_fails([
            "replay",
            "search-session-time",
            "--session",
            "race",
            "--time",
            "5:70"
        ]));
        assert!(command_parse_fails([
            "replay",
            "search-session-time",
            "--session",
            "race"
        ]));
        assert!(command_parse_fails([
            "replay",
            "search-session-time",
            "--time",
            "20"
        ]));
        assert!(command_parse_fails([
            "replay",
            "search-session-time",
            "--session",
            "99999",
            "--time",
            "20"
        ]));
        assert!(command_parse_fails([
            "replay",
            "search-session-time",
            "--session",
            "race",
            "--time-ms",
            "1000"
        ]));
    }

    #[test]
    fn search_session_time_dispatches_resolved_selector() -> Result<()> {
        let mut dependency = FakeBroadcast {
            sent: Vec::new(),
            sessions: Some(weekend_sessions()),
        };
        parse_command([
            "replay",
            "search-session-time",
            "--session",
            "race",
            "--time",
            "7:50",
        ])
        .run(&mut dependency)?;
        parse_command([
            "replay",
            "search-session-time",
            "--session",
            "0",
            "--time",
            "0",
        ])
        .run(&mut dependency)?;
        assert_eq!(
            dependency.sent,
            [
                BroadcastCommand::ReplaySearchSessionTime(2, 470_000),
                BroadcastCommand::ReplaySearchSessionTime(0, 0),
            ]
        );
        Ok(())
    }

    #[test]
    fn search_session_time_resolution_failure_prevents_dispatch() {
        let mut dependency = FakeBroadcast {
            sent: Vec::new(),
            sessions: Some(weekend_sessions()),
        };
        let error = parse_command([
            "replay",
            "search-session-time",
            "--session",
            "bogus",
            "--time",
            "1",
        ])
        .run(&mut dependency)
        .unwrap_err();
        assert!(error.to_string().contains("available sessions"));
        assert!(dependency.sent.is_empty());
    }
}
