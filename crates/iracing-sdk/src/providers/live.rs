use std::{marker::PhantomData, sync::Arc, time::Duration};

use crate::{
    FramePacket, Provider, Result, VariableSchema, WindowsConnection,
    runtime::{Timer, WaitRuntime},
    yaml_utils,
};

/// Live Windows shared-memory provider parameterized by runtime hooks.
pub struct LiveProvider<
    TimerRuntime = crate::runtime::DefaultTimer,
    WaitRuntimeImpl = crate::runtime::DefaultWaitRuntime,
> {
    connection: WindowsConnection,
    schema: Arc<VariableSchema>,
    timer: PhantomData<TimerRuntime>,
    wait_runtime: PhantomData<WaitRuntimeImpl>,
}

/// Default live provider for native Windows builds.
pub type DefaultLiveProvider =
    LiveProvider<crate::runtime::DefaultTimer, crate::runtime::DefaultWaitRuntime>;

impl<TimerRuntime, WaitRuntimeImpl> LiveProvider<TimerRuntime, WaitRuntimeImpl> {
    /// Connect to the default iRacing shared-memory source.
    pub fn new() -> Result<Self> {
        let connection = WindowsConnection::try_connect()?;

        Self::with_connection(connection)
    }

    /// Build a live provider from an existing Windows shared-memory connection.
    pub fn with_connection(connection: WindowsConnection) -> Result<Self> {
        let header = connection.header();
        tracing::info!(
            sdk_version = header.ver,
            tick_rate = header.tick_rate,
            num_vars = header.num_vars,
            status_connected = connection.is_connected(),
            "Connected to iRacing shared memory"
        );

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
            timer: PhantomData,
            wait_runtime: PhantomData,
        })
    }

    /// Get the variable schema.
    pub fn schema(&self) -> Arc<VariableSchema> {
        Arc::clone(&self.schema)
    }
}

#[async_trait::async_trait(?Send)]
impl<TimerRuntime, WaitRuntimeImpl> Provider for LiveProvider<TimerRuntime, WaitRuntimeImpl>
where
    TimerRuntime: Timer,
    WaitRuntimeImpl: WaitRuntime,
{
    async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
        let mut no_connection_count = 0u32;
        const MAX_NO_CONNECTION_ATTEMPTS: u32 = 600;
        const NO_CONNECTION_SLEEP: Duration = Duration::from_millis(500);

        loop {
            if !self.connection.is_connected() {
                no_connection_count += 1;

                if no_connection_count == 1 {
                    tracing::info!("Waiting for iRacing to start a session...");
                } else if no_connection_count.is_multiple_of(20) {
                    let elapsed = NO_CONNECTION_SLEEP.saturating_mul(no_connection_count);
                    tracing::debug!(
                        "Waiting for iRacing to start a session ({}s elapsed)",
                        elapsed.as_secs()
                    );
                }

                if no_connection_count >= MAX_NO_CONNECTION_ATTEMPTS {
                    tracing::warn!("Giving up after 5 minutes without iRacing session.");
                    return Ok(None);
                }

                TimerRuntime::sleep(NO_CONNECTION_SLEEP).await;
                continue;
            }

            if no_connection_count > 0 {
                tracing::info!("iRacing session detected, resuming telemetry aggregation.");
                no_connection_count = 0;
            }

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

            const TIMEOUT: Duration = Duration::from_millis(500);

            match WaitRuntimeImpl::wait_for_update(&self.connection, TIMEOUT).await? {
                crate::WaitResult::Signaled => {
                    tracing::trace!("Event signaled, checking for new data");
                    continue;
                }
                crate::WaitResult::Timeout => {
                    tracing::trace!("Wait timeout, continuing to poll");
                    continue;
                }
            }
        }
    }

    async fn session_yaml(&mut self, version: u32) -> Result<Option<String>> {
        tracing::debug!("Fetching session YAML from shared memory");

        let session_version = self.connection.session_info_update() as u32;
        if session_version == version {
            return Ok(None);
        }

        let raw_yaml = match self.connection.session_info() {
            Some(yaml) => yaml,
            None => {
                tracing::debug!("No session info available");
                return Ok(None);
            }
        };

        if raw_yaml.trim().is_empty() {
            return Ok(None);
        }

        let cleaned_yaml = yaml_utils::preprocess_iracing_yaml(&raw_yaml)?;

        tracing::info!("Extracted session YAML ({} bytes)", cleaned_yaml.len());
        Ok(Some(cleaned_yaml))
    }

    fn tick_rate(&self) -> f64 {
        self.connection.header().tick_rate as f64
    }
}
