//! Live telemetry connection for Windows

#[cfg(windows)]
use {
    crate::{
        FrameAdapter, LayoutProvider, Result, TelemetryLayout,
        provider::Provider,
        providers::live::LiveProvider,
        schema::SessionInfo,
        stream::ThrottleExt,
        telemetry::Telemetry,
        types::{FramePacket, UpdateRate},
    },
    futures::{Stream, StreamExt},
    std::sync::Arc,
    tokio::sync::watch,
    tokio_stream::wrappers::WatchStream,
    tokio_util::sync::CancellationToken,
};

/// Live connection to iRacing telemetry.
#[cfg(windows)]
pub struct LiveConnection {
    /// Frame receiver
    frames: watch::Receiver<Option<Arc<FramePacket>>>,

    /// Session receiver
    sessions: watch::Receiver<Option<Arc<SessionInfo>>>,

    /// Variable layout
    layout: Arc<TelemetryLayout>,

    /// Source frequency
    source_hz: f64,

    /// Cancellation token for stopping tasks
    cancel: CancellationToken,
}

#[cfg(windows)]
impl LiveConnection {
    /// Creates a new connection with a new provider.
    pub fn new() -> Result<Self> {
        let provider = LiveProvider::new()?;
        Ok(Self::from_provider(provider))
    }

    /// Starts background telemetry and session delivery from the provider.
    /// Retains its shared layout and source frequency for subscriptions.
    pub fn from_provider(provider: LiveProvider) -> Self {
        // Extract metadata
        let layout = provider.shared_layout();
        let source_hz = provider.tick_rate();

        // Spawn telemetry tasks
        let channels = Telemetry::spawn(provider);

        tracing::info!(
            "Live connection established ({}Hz) - waiting for iRacing session...",
            source_hz
        );

        Self {
            frames: channels.frames,
            sessions: channels.sessions,
            layout,
            source_hz,
            cancel: channels.cancel,
        }
    }

    /// Subscribes to the latest telemetry frames at the requested update rate.
    ///
    /// Intermediate frames may be skipped. The stream waits through initial
    /// empty updates and ends on an empty update after receiving a frame, or
    /// when the source channel closes.
    ///
    /// # Errors
    /// Propagates adapter layout validation errors before creating the stream.
    pub fn subscribe<T>(&self, rate: UpdateRate) -> Result<impl Stream<Item = T> + 'static>
    where
        T: FrameAdapter + Send + 'static,
    {
        // Validate layout at subscription time.
        let validation = T::validate_layout(&self.layout)?;

        // Create base frame stream from watch channel.
        //
        // Important: WatchStream yields the current value immediately. If no frames
        // have arrived yet, this will be None. We must handle this carefully to avoid
        // the stream appearing to end when it's actually just waiting for data.
        //
        // We skip initial None values to keep the stream alive while waiting for iRacing.
        // Once we receive our first frame, any subsequent None indicates the provider stopped.
        let frames = WatchStream::new(self.frames.clone())
            .skip_while(|opt| {
                // Skip leading `None` values (waiting for iRacing)
                let is_none = opt.is_none();
                async move { is_none }
            })
            .take_while(|opt| {
                // After skipping initial Nones, stop on the first None (provider ended)
                let is_some = opt.is_some();
                async move { is_some }
            })
            .filter_map(|opt| async move { opt });

        let stream = if let Some(interval) = rate.throttle_interval(self.source_hz) {
            frames
                .throttle(interval)
                .map(move |packet| T::adapt(&packet, &validation))
                .boxed()
        } else {
            frames
                .map(move |packet| T::adapt(&packet, &validation))
                .boxed()
        };

        Ok(stream)
    }

    /// Get session updates as a stream.
    pub fn session_updates(&self) -> impl Stream<Item = Arc<SessionInfo>> + 'static {
        WatchStream::new(self.sessions.clone()).filter_map(|opt| async move { opt })
    }

    /// Get current session info (if available)
    pub fn current_session(&self) -> Option<Arc<SessionInfo>> {
        self.sessions.borrow().clone()
    }

    /// Get the latest frame (if available)
    pub fn current_frame(&self) -> Option<Arc<FramePacket>> {
        self.frames.borrow().clone()
    }

    /// Get the source telemetry frequency
    pub fn source_hz(&self) -> f64 {
        self.source_hz
    }
}

#[cfg(windows)]
impl LayoutProvider for LiveConnection {
    /// Get the variable layout
    fn layout(&self) -> &Arc<TelemetryLayout> {
        &self.layout
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
/// Placeholder live connection type on unsupported platforms.
///
/// Calling [`Self::builder`] and building it returns
/// [`crate::IRacingSDKError::UnsupportedPlatform`].
pub struct LiveConnection {
    _private: (),
}
