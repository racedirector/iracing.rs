//! iracing-sdk broadcast SDK interface.
//! Wraps external `iracing-broadcast-sdk` types into `iracing-sdk` domain.
//!
use iracing_broadcast_sdk::Client;

use crate::{IRacingSDKError, Result};

/// Client for sending iRacing broadcast commands over the Win32 broadcast channel.
#[derive(Debug)]
pub struct Broadcast {
    client: Client,
}

impl Broadcast {
    /// Create a new broadcast client by registering the iRacing message ID.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError`] if `RegisterWindowMessageW` fails.
    pub fn new() -> Result<Self> {
        let client = Client::new()?;

        Ok(Self { client })
    }

    /// Send a typed broadcast message to iRacing.
    ///
    /// The command is packed into the `WPARAM`/`LPARAM` format expected by the
    /// official iRacing SDK and dispatched via `HWND_BROADCAST`.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError`] if the command cannot be encoded or if
    /// `SendNotifyMessageW` reports a Win32 error.
    pub fn send_message(&self, message: iracing_broadcast_sdk::Command) -> Result<()> {
        self.client
            .send_message(message)
            .map_err(IRacingSDKError::from)
    }
}
