//! Composition for the standalone `iracing-broadcast` executable.
//!
//! `main` creates this CLI-specific application and injects it into commands.
//! Commands declare `BroadcastCommands` and `ReplaySessions`; this module
//! lazily owns the shared `BroadcastClient` adapter and the live session
//! source.
//!
//! Construction is inert. The first use of a resource initializes only that
//! resource and caches success for the application's lifetime. Initialization
//! failures leave the slot empty so later calls retry. Operation errors retain
//! initialized resources. Exclusive mutable command injection serializes
//! initialization without locks. The non-Windows adapters report unsupported
//! use without Win32 setup.

use anyhow::Result;
use iracing_broadcast_cli::{BroadcastClient, BroadcastCommands, ReplaySession, ReplaySessions};
use iracing_broadcast_sdk::Command;

// Private factory/resource parameters let tests count initialization without Win32.
// Production uses the concrete defaults; this is not a service registry.
pub(crate) struct Application<
    B = BroadcastClient,
    S = LiveSessionSource,
    F = fn() -> Result<B>,
    G = fn() -> Result<S>,
> {
    broadcast: Option<B>,
    sessions: Option<S>,
    initialize_broadcast: F,
    initialize_sessions: G,
}

impl Application {
    /// Create an application without initializing any simulator resources.
    pub(crate) fn new() -> Self {
        Self::with_initializers(BroadcastClient::new, LiveSessionSource::try_connect)
    }
}

impl<B, S, F: FnMut() -> Result<B>, G: FnMut() -> Result<S>> Application<B, S, F, G> {
    /// Retain the resource initializers, calling each only when its resource
    /// is first used.
    fn with_initializers(initialize_broadcast: F, initialize_sessions: G) -> Self {
        Self {
            broadcast: None,
            sessions: None,
            initialize_broadcast,
            initialize_sessions,
        }
    }

    /// Initialize the broadcast client on first use.
    ///
    /// Propagates initialization errors and leaves the resource absent so the
    /// next call retries. Does not initialize the session source.
    fn broadcast(&mut self) -> Result<&mut B> {
        match &mut self.broadcast {
            Some(client) => Ok(client),
            slot @ None => Ok(slot.insert((self.initialize_broadcast)()?)),
        }
    }

    /// Initialize the session source on first use.
    ///
    /// Propagates initialization errors and leaves the resource absent so the
    /// next call retries. Does not initialize the broadcast client.
    fn sessions(&mut self) -> Result<&mut S> {
        match &mut self.sessions {
            Some(source) => Ok(source),
            slot @ None => Ok(slot.insert((self.initialize_sessions)()?)),
        }
    }
}

impl<B: BroadcastCommands, S, F: FnMut() -> Result<B>, G: FnMut() -> Result<S>> BroadcastCommands
    for Application<B, S, F, G>
{
    /// Initialize the broadcast client on first use and dispatch the command.
    ///
    /// Propagates initialization and dispatch errors. Failed initialization is
    /// retried on the next call; dispatch errors retain the initialized client.
    fn send_broadcast(&mut self, command: Command) -> Result<()> {
        self.broadcast()?.send_broadcast(command)
    }
}

impl<S: SessionSource, B, F: FnMut() -> Result<B>, G: FnMut() -> Result<S>> ReplaySessions
    for Application<B, S, F, G>
{
    fn replay_sessions(&mut self) -> Result<Vec<ReplaySession>> {
        self.sessions()?.live_replay_sessions()
    }
}

/// A source able to snapshot the sessions published by the simulator.
///
/// Implementations own live session acquisition; tests script the snapshot
/// without Win32.
trait SessionSource {
    /// Return the sessions currently published by the simulator.
    ///
    /// # Errors
    ///
    /// Propagates acquisition and parsing failures, and reports unsupported
    /// platforms.
    fn live_replay_sessions(&mut self) -> Result<Vec<ReplaySession>>;
}

/// Live session source reading the simulator's shared-memory session
/// metadata.
#[cfg(windows)]
pub(crate) struct LiveSessionSource {
    connection: iracing_sdk::WindowsConnection,
}

#[cfg(windows)]
impl LiveSessionSource {
    /// Open live shared memory without waiting for the simulator to publish
    /// session metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if telemetry is not connected. Propagates
    /// shared-memory setup errors.
    fn try_connect() -> Result<Self> {
        let connection = match iracing_sdk::WindowsConnection::try_connect() {
            Ok(connection) if connection.is_connected() => connection,
            Ok(_) => {
                return Err(anyhow::anyhow!(
                    "Shared memory opened but telemetry is not connected yet"
                ));
            }
            Err(error) => return Err(anyhow::anyhow!(error)),
        };
        Ok(Self { connection })
    }
}

#[cfg(windows)]
impl SessionSource for LiveSessionSource {
    /// Read the current session metadata; an absent snapshot is an empty
    /// list, which the command layer then reports to the operator.
    fn live_replay_sessions(&mut self) -> Result<Vec<ReplaySession>> {
        use iracing_sdk::provider::SessionInformationProvider;

        let Some(session_info) = self.connection.session_info()? else {
            return Ok(Vec::new());
        };
        session_info
            .session_info
            .sessions
            .iter()
            .map(ReplaySession::from_live_session)
            .collect()
    }
}

#[cfg(not(windows))]
pub(crate) struct LiveSessionSource;

#[cfg(not(windows))]
impl LiveSessionSource {
    /// Return an unsupported-platform error without opening a connection.
    fn try_connect() -> Result<Self> {
        anyhow::bail!("Live telemetry only runs on Windows")
    }
}

#[cfg(not(windows))]
impl SessionSource for LiveSessionSource {
    fn live_replay_sessions(&mut self) -> Result<Vec<ReplaySession>> {
        anyhow::bail!("Live telemetry only runs on Windows")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    struct FakeBroadcast {
        sends: Rc<Cell<usize>>,
    }
    impl BroadcastCommands for FakeBroadcast {
        fn send_broadcast(&mut self, _: Command) -> Result<()> {
            self.sends.set(self.sends.get() + 1);
            anyhow::bail!("dispatch failed")
        }
    }

    struct FakeSessions {
        snapshot_calls: Rc<Cell<usize>>,
    }
    impl SessionSource for FakeSessions {
        fn live_replay_sessions(&mut self) -> Result<Vec<ReplaySession>> {
            self.snapshot_calls.set(self.snapshot_calls.get() + 1);
            Ok(vec![ReplaySession {
                number: 2,
                session_type: "Race".to_string(),
                name: None,
            }])
        }
    }

    fn unused_broadcast_initializer() -> impl FnMut() -> Result<FakeBroadcast> {
        || {
            anyhow::bail!("broadcast initialization should not run");
        }
    }

    #[test]
    fn construction_is_inert_and_success_is_reused_even_after_dispatch_error() {
        let initializations = Rc::new(Cell::new(0));
        let sends = Rc::new(Cell::new(0));
        let mut application = Application::with_initializers(
            {
                let initializations = initializations.clone();
                let sends = sends.clone();
                move || {
                    initializations.set(initializations.get() + 1);
                    Ok(FakeBroadcast {
                        sends: sends.clone(),
                    })
                }
            },
            unused_broadcast_initializer(),
        );
        assert_eq!(initializations.get(), 0);
        for _ in 0..2 {
            assert!(
                application
                    .send_broadcast(Command::ReloadAllTextures)
                    .is_err()
            );
        }
        assert_eq!(initializations.get(), 1);
        assert_eq!(sends.get(), 2);
    }

    #[test]
    fn failed_initialization_retries() {
        let attempts = Cell::new(0);
        let mut application = Application::with_initializers(
            {
                let attempts = &attempts;
                move || {
                    attempts.set(attempts.get() + 1);
                    if attempts.get() == 1 {
                        anyhow::bail!("initialization failed");
                    }
                    Ok(FakeBroadcast {
                        sends: Rc::new(Cell::new(0)),
                    })
                }
            },
            unused_broadcast_initializer(),
        );
        assert!(
            application
                .send_broadcast(Command::ReloadAllTextures)
                .is_err()
        );
        assert_eq!(attempts.get(), 1);
        assert!(
            application
                .send_broadcast(Command::ReloadAllTextures)
                .is_err()
        );
        assert!(
            application
                .send_broadcast(Command::ReloadAllTextures)
                .is_err()
        );
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn session_source_is_lazy_independent_and_success_is_reused() {
        let broadcast_inits = Rc::new(Cell::new(0));
        let session_inits = Rc::new(Cell::new(0));
        let snapshot_calls = Rc::new(Cell::new(0));
        let mut application = Application::with_initializers(
            {
                let broadcast_inits = broadcast_inits.clone();
                move || {
                    broadcast_inits.set(broadcast_inits.get() + 1);
                    Ok(FakeBroadcast {
                        sends: Rc::new(Cell::new(0)),
                    })
                }
            },
            {
                let session_inits = session_inits.clone();
                let snapshot_calls = snapshot_calls.clone();
                move || {
                    session_inits.set(session_inits.get() + 1);
                    Ok(FakeSessions {
                        snapshot_calls: snapshot_calls.clone(),
                    })
                }
            },
        );
        assert_eq!(broadcast_inits.get(), 0);
        assert_eq!(session_inits.get(), 0);

        let sessions = application.replay_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].number, 2);
        assert!(application.replay_sessions().is_ok());
        assert_eq!(session_inits.get(), 1);
        assert_eq!(snapshot_calls.get(), 2);
        assert_eq!(broadcast_inits.get(), 0);

        application
            .send_broadcast(Command::ReloadAllTextures)
            .unwrap_err();
        assert_eq!(broadcast_inits.get(), 1);
    }

    #[test]
    fn session_initialization_failure_retries_then_success_is_reused() {
        let attempts = Rc::new(Cell::new(0));
        let snapshot_calls = Rc::new(Cell::new(0));
        let mut application = Application::with_initializers(unused_broadcast_initializer(), {
            let attempts = attempts.clone();
            let snapshot_calls = snapshot_calls.clone();
            move || {
                attempts.set(attempts.get() + 1);
                if attempts.get() == 1 {
                    anyhow::bail!("session initialization failed");
                }
                Ok(FakeSessions {
                    snapshot_calls: snapshot_calls.clone(),
                })
            }
        });
        assert!(application.replay_sessions().is_err());
        assert_eq!(attempts.get(), 1);
        assert!(application.replay_sessions().is_ok());
        assert!(application.replay_sessions().is_ok());
        assert_eq!(attempts.get(), 2);
        assert_eq!(snapshot_calls.get(), 2);
    }

    #[test]
    fn production_construction_is_inert() {
        let application = Application::new();
        assert!(application.broadcast.is_none());
        assert!(application.sessions.is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn production_dispatch_is_unsupported() {
        assert!(
            Application::new()
                .send_broadcast(Command::ReloadAllTextures)
                .unwrap_err()
                .to_string()
                // See L135 in lib.rs for the error message
                .contains("only run on Windows")
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn production_replay_sessions_are_unsupported() {
        assert!(
            Application::new()
                .replay_sessions()
                .unwrap_err()
                .to_string()
                // See L172 for the error message
                .contains("only runs on Windows")
        );
    }
}
