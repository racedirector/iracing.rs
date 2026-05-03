use std::{sync::Arc, time::Duration};

use crate::{
    FramePacket, Provider, Result, VariableSchema, WaitResult, WindowsConnection, yaml_utils,
};

/// A [`Provider`] that streams telemetry frames from an iRacing mmap file.
pub struct LiveProvider {
    connection: WindowsConnection,
    schema: Arc<VariableSchema>,
}

impl LiveProvider {
    /// Opens a connection the iRacing live telemetry and constructs a `LiveProvider`.
    pub fn new() -> Result<Self> {
        let connection = WindowsConnection::try_connect()?;

        Self::with_connection(connection)
    }

    /// Constructs a `LiveProvider` from an established [`WindowsConnection`].
    pub fn with_connection(connection: WindowsConnection) -> Result<Self> {
        let header = connection.header();
        let variables = connection.get_variables();
        let mut variable_map = std::collections::HashMap::new();

        for var_info in variables {
            variable_map.insert(var_info.name.clone(), var_info);
        }

        let frame_size = header.buf_len as usize;
        let schema = Arc::new(VariableSchema::new(variable_map, frame_size)?);

        Ok(Self { connection, schema })
    }

    /// Get the variable schema
    pub fn schema(&self) -> Arc<VariableSchema> {
        Arc::clone(&self.schema)
    }
}

#[async_trait::async_trait]
impl Provider for LiveProvider {
    async fn next_frame(&mut self) -> Result<Option<crate::FramePacket>> {
        // Track how long we've been waiting without a connection
        let mut no_connection_count = 0u32;
        const MAX_NO_CONNECTION_ATTEMPTS: u32 = 600; // 5 minutes at 500ms intervals

        // Loop until we get a frame
        // This matches the C++ SDK pattern of persistent checking
        loop {
            // Check if still connected (like C++ SDK checks status)
            if !self.connection.is_connected() {
                no_connection_count += 1;

                // Log periodically to avoid spam
                if no_connection_count == 1 {
                    tracing::info!("Waiting for iRacing to start a session...");
                } else if no_connection_count.is_multiple_of(20) {
                    tracing::debug!(
                        "Still waiting for iRacing session ({}s elapsed)",
                        no_connection_count / 2
                    );
                }

                // Give up after extended period with no connection
                if no_connection_count >= MAX_NO_CONNECTION_ATTEMPTS {
                    tracing::warn!("Giving up after 5 minutes without iRacing session");
                    return Ok(None);
                }

                // Wait a bit before checking again
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }

            // Reset counter when we get a connection
            if no_connection_count > 0 {
                tracing::info!("iRacing session detected, resuming telemetry");
                no_connection_count = 0;
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
            const TIMEOUT: Duration = Duration::from_millis(500);

            match self.connection.wait_for_update_async(TIMEOUT).await? {
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

    async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
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

    fn tick_rate(&self) -> f64 {
        self.connection.header().tick_rate as f64
    }
}
