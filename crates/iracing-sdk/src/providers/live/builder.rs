use std::time::Duration;

use crate::{IRacingSDKError, Result, WindowsConnection};

use super::LiveProvider;

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_MAX_NO_CONNECTION_ATTEMPTS: u32 = 600;

/// Builder for a configurable [`LiveProvider`].
///
/// The default configuration polls every 500 milliseconds and ends the provider
/// stream after 600 consecutive observations without an active iRacing session.
#[derive(Debug)]
pub struct LiveProviderBuilder {
    pub(super) connection: Option<WindowsConnection>,
    pub(super) poll_interval: Duration,
    pub(super) max_no_connection_attempts: Option<u32>,
}

impl Default for LiveProviderBuilder {
    fn default() -> Self {
        Self {
            connection: None,
            poll_interval: DEFAULT_POLL_INTERVAL,
            max_no_connection_attempts: Some(DEFAULT_MAX_NO_CONNECTION_ATTEMPTS),
        }
    }
}

impl LiveProviderBuilder {
    /// Use an already-established Windows shared-memory connection.
    pub fn with_connection(mut self, connection: WindowsConnection) -> Self {
        self.connection = Some(connection);
        self
    }

    /// Set how long the provider waits between disconnected-session checks and
    /// for connected telemetry update events.
    ///
    /// A zero duration is rejected by [`Self::build`].
    pub fn with_poll_interval(mut self, poll_interval: Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }

    /// Set the maximum number of consecutive disconnected observations.
    ///
    /// A value of `1` makes `Provider::next_frame` return `Ok(None)` immediately
    /// on the first disconnected observation. A value of zero is rejected by
    /// [`Self::build`].
    pub fn with_max_no_connection_attempts(mut self, attempts: u32) -> Self {
        self.max_no_connection_attempts = Some(attempts);
        self
    }

    /// Wait indefinitely for an iRacing session to become active.
    pub fn without_no_connection_limit(mut self) -> Self {
        self.max_no_connection_attempts = None;
        self
    }

    /// Build the configured live telemetry provider.
    ///
    /// If no connection was supplied with [`Self::with_connection`], this opens
    /// the iRacing Windows shared-memory connection during the build.
    pub fn build(self) -> Result<LiveProvider> {
        if self.poll_interval.is_zero() {
            return Err(IRacingSDKError::invalid_configuration(
                "poll_interval",
                "must be greater than zero",
            ));
        }

        if self.max_no_connection_attempts == Some(0) {
            return Err(IRacingSDKError::invalid_configuration(
                "max_no_connection_attempts",
                "must be greater than zero when a finite limit is configured",
            ));
        }

        let connection = match self.connection {
            Some(connection) => connection,
            None => WindowsConnection::try_connect()?,
        };

        LiveProvider::from_parts(
            connection,
            self.poll_interval,
            self.max_no_connection_attempts,
        )
    }
}
