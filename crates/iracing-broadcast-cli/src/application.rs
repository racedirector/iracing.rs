//! Composition for the standalone `iracing-broadcast` executable.
//!
//! `main` creates this CLI-specific application and injects it into commands.
//! Commands declare `BroadcastCommands`; this module lazily owns the shared
//! `BroadcastClient` adapter. No telemetry dependency is needed by this executable.
//!
//! Construction is inert. The first dispatch initializes the client and caches
//! success for the application's lifetime. Initialization failures leave the slot
//! empty so later calls retry. Dispatch errors retain an initialized client.
//! Exclusive mutable command injection serializes initialization without locks.
//! The non-Windows adapter reports unsupported dispatch without Win32 setup.

use anyhow::Result;
use iracing_broadcast_cli::{BroadcastClient, BroadcastCommands};
use iracing_broadcast_sdk::Command;

// Private factory/resource parameters let tests count initialization without Win32.
// Production uses the concrete defaults; this is not a service registry.
pub(crate) struct Application<B = BroadcastClient, F = fn() -> Result<B>> {
    broadcast: Option<B>,
    initialize_broadcast: F,
}

impl Application {
    pub(crate) fn new() -> Self {
        Self::with_initializer(BroadcastClient::new)
    }
}

impl<B, F: FnMut() -> Result<B>> Application<B, F> {
    fn with_initializer(initialize_broadcast: F) -> Self {
        Self {
            broadcast: None,
            initialize_broadcast,
        }
    }
}

impl<B: BroadcastCommands, F: FnMut() -> Result<B>> BroadcastCommands for Application<B, F> {
    fn send_broadcast(&mut self, command: Command) -> Result<()> {
        let client = match &mut self.broadcast {
            Some(client) => client,
            slot @ None => slot.insert((self.initialize_broadcast)()?),
        };
        client.send_broadcast(command)
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

    #[test]
    fn construction_is_inert_and_success_is_reused_even_after_dispatch_error() {
        let initializations = Rc::new(Cell::new(0));
        let sends = Rc::new(Cell::new(0));
        let mut application = Application::with_initializer({
            let initializations = initializations.clone();
            let sends = sends.clone();
            move || {
                initializations.set(initializations.get() + 1);
                Ok(FakeBroadcast {
                    sends: sends.clone(),
                })
            }
        });
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
        let mut application = Application::with_initializer(|| {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 1 {
                anyhow::bail!("initialization failed");
            }
            Ok(FakeBroadcast {
                sends: Rc::new(Cell::new(0)),
            })
        });
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
    fn production_construction_is_inert() {
        let application = Application::new();
        assert!(application.broadcast.is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn production_dispatch_is_unsupported() {
        assert!(
            Application::new()
                .send_broadcast(Command::ReloadAllTextures)
                .unwrap_err()
                .to_string()
                .contains("only run on Windows")
        );
    }
}
