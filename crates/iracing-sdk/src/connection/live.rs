//! Live telemetry connection for Windows

use crate::Result;

#[cfg(windows)]
use {
    crate::emitter::TelemetryEmitter,
    crate::providers::LiveProvider,
    crate::providers::Provider,
    crate::stream::ThrottleExt,
    crate::types::{FramePacket, UpdateRate},
    crate::{FrameAdapter, SessionInfo, VariableSchema},
    futures::{Stream, StreamExt},
    std::sync::Arc,
    std::time::Duration,
    tokio::sync::watch,
    tokio_stream::wrappers::WatchStream,
    tokio_util::sync::CancellationToken,
};

/// Live connection to iRacing telemetry
#[cfg(windows)]
pub struct LiveConnection {
    /// Frame watch receiver
    frames: watch::Receiver<Option<Arc<FramePacket>>>,

    /// Session watch receiver
    sessions: watch::Receiver<Option<Arc<SessionInfo>>>,

    /// Variable schema
    schema: Arc<VariableSchema>,

    /// Source frequency
    source_hz: f64,

    /// Cancellation token for stopping tasks
    cancel: CancellationToken,
}

#[cfg(windows)]
impl LiveConnection {
    /// Create a new live connection.
    ///
    /// This method establishes a connection to iRacing's shared memory and starts
    /// monitoring for telemetry data. The connection will wait for iRacing to
    /// start a session before streaming frames.
    pub async fn connect() -> Result<Self> {
        tracing::info!("Connecting to iRacing live telemetry");

        // Create provider and extract metadata
        let provider = LiveProvider::new()?;
        Self::with_provider(provider).await
    }

    /// Creates a Live connection with a given provider.
    pub async fn with_provider(provider: LiveProvider) -> Result<Self> {
        let schema = provider.schema();
        let source_hz = provider.tick_rate();

        // Spawn driver tasks - they will wait for iRacing to start
        let channels = TelemetryEmitter::spawn(provider);

        // Don't wait for frames here - let the streams handle waiting
        // This allows the connection to be established even if iRacing isn't
        // in a session yet. The streams will wait for data.

        tracing::info!(
            "Live connection established ({}Hz) - waiting for iRacing session",
            source_hz
        );

        Ok(Self {
            frames: channels.frames,
            sessions: channels.sessions,
            schema,
            source_hz,
            cancel: channels.cancel,
        })
    }

    /// Subscribe to telemetry frames
    pub fn subscribe<T>(&self, rate: UpdateRate) -> impl Stream<Item = T> + 'static
    where
        T: FrameAdapter + Send + 'static,
    {
        // Validate schema once at subscription time
        let validation = T::validate_schema(&self.schema).expect("Schema validation failed");

        // Create base frame stream from watch channel
        // Important: WatchStream yields the current value immediately. If no frames
        // have arrived yet, this will be None. We must handle this carefully to avoid
        // the stream appearing to end when it's actually just waiting for data.
        //
        // We skip initial None values to keep the stream alive while waiting for iRacing.
        // Once we receive our first frame, any subsequent None indicates the provider stopped.
        let frames = WatchStream::new(self.frames.clone())
            .skip_while(|opt| {
                // Skip leading None values (waiting for iRacing)
                let is_none = opt.is_none();
                async move { is_none }
            })
            .take_while(|opt| {
                // After skipping initial Nones, stop on the first None (provider ended)
                let is_some = opt.is_some();
                async move { is_some }
            })
            .filter_map(|opt| async move { opt });

        // Apply rate control and adaptation
        let effective_rate = rate.normalize(self.source_hz);

        match effective_rate {
            UpdateRate::Native => {
                // Direct adaptation, no throttling
                frames
                    .map(move |packet| T::adapt(&packet, &validation))
                    .boxed()
            }
            UpdateRate::Max(hz) => {
                // Throttle then adapt
                let interval = Duration::from_secs_f64(1.0 / hz as f64);
                frames
                    .throttle(interval)
                    .map(move |packet| T::adapt(&packet, &validation))
                    .boxed()
            }
        }
    }

    /// Get session updates as a stream
    ///
    /// Sessions are automatically detected by the Driver when session versions
    /// change, and YAML is parsed asynchronously without blocking frame processing.
    ///
    /// This stream will emit the current session immediately (if available), then
    /// emit subsequent session changes.
    ///
    /// Uses WatchStream which automatically handles initial state correctly:
    /// - Yields current session on subscription (if any)
    /// - Yields subsequent updates as they arrive
    /// - No manual skip/dedup needed - watch channel semantics handle it
    pub fn session_updates(&self) -> impl Stream<Item = Arc<SessionInfo>> + 'static {
        WatchStream::new(self.sessions.clone()).filter_map(|opt| async move { opt })
    }

    /// Get current session info (if any)
    pub fn current_session(&self) -> Option<Arc<SessionInfo>> {
        self.sessions.borrow().clone()
    }

    /// Get the source telemetry frequency
    pub fn source_hz(&self) -> f64 {
        self.source_hz
    }

    /// Get the variable schema
    pub fn schema(&self) -> &VariableSchema {
        &self.schema
    }
}

#[cfg(windows)]
impl Drop for LiveConnection {
    fn drop(&mut self) {
        tracing::debug!("Dropping live connection");
        // Cancel tasks on drop for clean shutdown
        self.cancel.cancel();
    }
}

// Non-Windows stub implementation
#[cfg(not(windows))]
pub struct LiveConnection {
    _private: (),
}

#[cfg(not(windows))]
impl LiveConnection {
    /// Attempt to create a live connection on non-Windows platforms.
    ///
    /// This always returns an error as live telemetry is only available on Windows.
    /// Consider using `Pitwall::open()` with an IBT file for cross-platform testing.
    pub async fn connect() -> Result<Self> {
        Err(crate::IRacingSDKError::unsupported_platform(
            "Live telemetry",
            "Windows",
        ))
    }
}
