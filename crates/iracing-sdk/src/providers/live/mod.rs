//! Live telemetry provider for Windows

mod builder;

pub use builder::LiveProviderBuilder;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    FramePacket, Result, VariableSchema, WaitResult, WindowsConnection, provider::Provider,
    yaml_utils,
};

const WAITING_LOG_INTERVAL: Duration = Duration::from_secs(10);

/// A [`Provider`] that streams telemetry frames from an iRacing mmap file.
#[derive(Debug)]
pub struct LiveProvider {
    connection: WindowsConnection,
    schema: Arc<VariableSchema>,
    poll_interval: Duration,
    max_no_connection_attempts: Option<u32>,
}

impl LiveProvider {
    /// Start configuring a `LiveProvider`.
    ///
    /// ```no_run
    /// # #[cfg(windows)]
    /// # {
    /// use std::time::Duration;
    /// use iracing_sdk::providers::live::LiveProvider;
    ///
    /// let provider = LiveProvider::builder()
    ///     .with_poll_interval(Duration::from_secs(1))
    ///     .with_max_no_connection_attempts(900)
    ///     .build()?;
    /// # let _ = provider;
    /// # Ok::<(), iracing_sdk::IRacingSDKError>(())
    /// # }
    /// # #[cfg(not(windows))]
    /// # Ok::<(), iracing_sdk::IRacingSDKError>(())
    /// ```
    pub fn builder() -> LiveProviderBuilder {
        LiveProviderBuilder::default()
    }

    /// Opens a connection the iRacing live telemetry and constructs a `LiveProvider`.
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    fn from_parts(
        connection: WindowsConnection,
        poll_interval: Duration,
        max_no_connection_attempts: Option<u32>,
    ) -> Result<Self> {
        let header = connection.header();
        let variables = connection.get_variables();
        let mut variable_map = std::collections::HashMap::new();

        for var_info in variables {
            variable_map.insert(var_info.name.clone(), var_info);
        }

        let frame_size = header.buf_len as usize;
        let schema = Arc::new(VariableSchema::new(variable_map, frame_size)?);

        Ok(Self {
            connection,
            schema,
            poll_interval,
            max_no_connection_attempts,
        })
    }

    /// Get the variable schema
    pub fn schema(&self) -> Arc<VariableSchema> {
        Arc::clone(&self.schema)
    }

    async fn next_frame_impl(&mut self) -> Result<Option<FramePacket>> {
        let mut no_connection = NoConnectionState::default();

        // Loop until we get a frame
        // This matches the C++ SDK pattern of persistent checking
        loop {
            // Check if still connected (like C++ SDK checks status)
            if !self.connection.is_connected() {
                let now = Instant::now();
                let first_observation = no_connection.observe(now);

                // Log periodically to avoid spam
                if first_observation {
                    tracing::info!("Waiting for iRacing to start a session...");
                } else if no_connection.should_log_progress(now) {
                    tracing::debug!(
                        elapsed_seconds = no_connection.elapsed(now).as_secs_f64(),
                        attempts = no_connection.attempts,
                        "Still waiting for iRacing session"
                    );
                }

                if no_connection.exhausted(self.max_no_connection_attempts) {
                    tracing::warn!(
                        elapsed_seconds = no_connection.elapsed(now).as_secs_f64(),
                        attempts = no_connection.attempts,
                        max_no_connection_attempts = ?self.max_no_connection_attempts,
                        poll_interval_ms = self.poll_interval.as_millis(),
                        "Giving up without an active iRacing session"
                    );
                    return Ok(None);
                }

                // Wait a bit before checking again
                tokio::time::sleep(self.poll_interval).await;
                continue;
            }

            // Reset counter when we get a connection
            if no_connection.reset() {
                tracing::info!("iRacing session detected, resuming telemetry");
            }

            // Try to get data BEFORE waiting (C++ SDK pattern)
            // This catches frames that arrived since our last check
            if let Some(data) = self.connection.get_new_data() {
                let frame_data = data.to_vec();
                let header = self.connection.header();
                let latest_buf_idx = self.connection.find_latest_buffer(header);
                let tick = header.var_buf[latest_buf_idx].tick_count as u32;
                let session_version = header.session_info_update as u32;

                tracing::trace!(
                    "Frame: tick={}, session_version={}, size={}",
                    tick,
                    session_version,
                    frame_data.len()
                );

                return Ok(Some(FramePacket::new(
                    frame_data,
                    tick,
                    session_version,
                    Arc::clone(&self.schema),
                )));
            }

            // No data yet, wait for signal (cooperative async)
            match self
                .connection
                .wait_for_update_async(self.poll_interval)
                .await?
            {
                WaitResult::Signaled => {
                    // Event fired, loop back to check for data
                    // The event might be for session info or a frame we haven't
                    // seen yet due to tick count not changing
                    tracing::trace!("Event signaled, checking for new data");
                    continue;
                }
                WaitResult::Timeout => {
                    // No event within timeout, but keep trying
                    // Live streams don't end unless disconnected
                    tracing::trace!("Wait timeout, continuing to poll");
                    continue;
                }
            }
        }
    }

    async fn session_yaml_impl(&mut self, _version: u32) -> Result<Option<String>> {
        tracing::debug!("Fetching session YAML from shared memory");

        // Get raw YAML from shared memory
        let raw_yaml = match self.connection.session_info() {
            Some(yaml) => yaml,
            None => {
                tracing::debug!("No session info available");
                return Ok(None);
            }
        };

        // Return None if empty
        if raw_yaml.trim().is_empty() {
            return Ok(None);
        }

        // Preprocess to fix iRacing's YAML issues
        let cleaned_yaml = yaml_utils::preprocess_iracing_yaml(&raw_yaml)?;

        tracing::info!("Extracted session YAML ({} bytes)", cleaned_yaml.len());

        Ok(Some(cleaned_yaml))
    }
}

#[derive(Debug, Default)]
struct NoConnectionState {
    attempts: u32,
    started_at: Option<Instant>,
    last_progress_log_at: Option<Instant>,
}

impl NoConnectionState {
    fn observe(&mut self, now: Instant) -> bool {
        let first_observation = self.attempts == 0;
        self.attempts = self.attempts.saturating_add(1);

        if first_observation {
            self.started_at = Some(now);
            self.last_progress_log_at = Some(now);
        }

        first_observation
    }

    fn exhausted(&self, max_attempts: Option<u32>) -> bool {
        max_attempts.is_some_and(|max_attempts| self.attempts >= max_attempts)
    }

    fn elapsed(&self, now: Instant) -> Duration {
        self.started_at
            .map_or(Duration::ZERO, |started_at| now.duration_since(started_at))
    }

    fn should_log_progress(&mut self, now: Instant) -> bool {
        let should_log = self
            .last_progress_log_at
            .is_none_or(|last_log| now.duration_since(last_log) >= WAITING_LOG_INTERVAL);

        if should_log {
            self.last_progress_log_at = Some(now);
        }

        should_log
    }

    fn reset(&mut self) -> bool {
        let had_observations = self.attempts > 0;
        *self = Self::default();
        had_observations
    }
}

#[async_trait::async_trait]
impl Provider for LiveProvider {
    async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
        self.next_frame_impl().await
    }

    async fn session_yaml(&mut self, version: u32) -> Result<Option<String>> {
        self.session_yaml_impl(version).await
    }

    fn tick_rate(&self) -> f64 {
        self.connection.header().tick_rate as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IRacingSDKError;

    #[test]
    fn builder_uses_legacy_defaults() {
        let builder = LiveProvider::builder();

        assert_eq!(builder.poll_interval, Duration::from_millis(500));
        assert_eq!(builder.max_no_connection_attempts, Some(600));
        assert!(builder.connection.is_none());
    }

    #[test]
    fn builder_applies_connection_policy_overrides() {
        let builder = LiveProvider::builder()
            .with_poll_interval(Duration::from_secs(2))
            .with_max_no_connection_attempts(42);

        assert_eq!(builder.poll_interval, Duration::from_secs(2));
        assert_eq!(builder.max_no_connection_attempts, Some(42));

        let builder = builder.without_no_connection_limit();
        assert_eq!(builder.max_no_connection_attempts, None);
    }

    #[test]
    fn builder_rejects_zero_poll_interval_before_connecting() {
        let error = LiveProvider::builder()
            .with_poll_interval(Duration::ZERO)
            .build()
            .expect_err("zero polling interval must be rejected");

        assert!(matches!(
            error,
            IRacingSDKError::InvalidConfiguration {
                field: "poll_interval",
                ..
            }
        ));
    }

    #[test]
    fn builder_rejects_zero_max_attempts_before_connecting() {
        let error = LiveProvider::builder()
            .with_max_no_connection_attempts(0)
            .build()
            .expect_err("zero maximum must be rejected");

        assert!(matches!(
            error,
            IRacingSDKError::InvalidConfiguration {
                field: "max_no_connection_attempts",
                ..
            }
        ));
    }

    #[test]
    fn no_connection_limit_is_checked_on_each_observation() {
        let mut state = NoConnectionState::default();
        let now = Instant::now();

        assert!(state.observe(now));
        assert!(state.exhausted(Some(1)));

        let mut state = NoConnectionState::default();
        assert!(state.observe(now));
        assert!(!state.exhausted(Some(3)));
        assert!(!state.observe(now));
        assert!(!state.exhausted(Some(3)));
        assert!(!state.observe(now));
        assert!(state.exhausted(Some(3)));
    }

    #[test]
    fn unlimited_policy_never_exhausts() {
        let mut state = NoConnectionState::default();
        let now = Instant::now();

        for _ in 0..1_000 {
            state.observe(now);
            assert!(!state.exhausted(None));
        }
    }

    #[test]
    fn reconnect_resets_no_connection_state() {
        let mut state = NoConnectionState::default();
        let now = Instant::now();

        state.observe(now);
        state.observe(now);
        assert!(state.reset());
        assert_eq!(state.attempts, 0);
        assert_eq!(state.started_at, None);
        assert_eq!(state.last_progress_log_at, None);
        assert!(!state.reset());
        assert!(state.observe(now));
    }

    #[test]
    fn progress_logging_is_elapsed_time_based() {
        let mut state = NoConnectionState::default();
        let started_at = Instant::now();

        state.observe(started_at);
        assert!(!state.should_log_progress(started_at + Duration::from_secs(9)));
        assert!(state.should_log_progress(started_at + Duration::from_secs(10)));
        assert!(!state.should_log_progress(started_at + Duration::from_secs(19)));
        assert!(state.should_log_progress(started_at + Duration::from_secs(20)));
    }
}
