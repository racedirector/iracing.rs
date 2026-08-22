//! Live telemetry connection for Windows

mod builder;

pub use builder::LiveConnectionBuilder;

#[cfg(windows)]
use crate::SchemaProvider;

#[cfg(windows)]
use {
    crate::{
        FrameAdapter, Result, VariableSchema,
        provider::Provider,
        providers::live::LiveProvider,
        schema::SessionInfo,
        stream::ThrottleExt,
        telemetry::Telemetry,
        types::{FramePacket, UpdateRate},
    },
    futures::{Stream, StreamExt},
    std::{sync::Arc, time::Duration},
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

    /// Variable schema
    schema: Arc<VariableSchema>,

    /// Source frequency
    source_hz: f64,

    /// Cancellation token for stopping tasks
    cancel: CancellationToken,
}

#[cfg(windows)]
impl LiveConnection {
    /// Start building a live telemetry connection.
    pub fn builder() -> LiveConnectionBuilder {
        LiveConnectionBuilder::default()
    }

    fn from_provider(provider: LiveProvider) -> Self {
        // Extract metadata
        let schema = provider.shared_schema();
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
            schema,
            source_hz,
            cancel: channels.cancel,
        }
    }

    /// Subscribe to telemetry frames
    pub fn subscribe<T>(&self, rate: UpdateRate) -> Result<impl Stream<Item = T> + 'static>
    where
        T: FrameAdapter + Send + 'static,
    {
        // Validate schema at subscription time.
        let validation = T::validate_schema(&self.schema)?;

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
impl SchemaProvider for LiveConnection {
    /// Get the variable schema
    fn schema(&self) -> &VariableSchema {
        self.schema.as_ref()
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

#[cfg(not(windows))]
impl LiveConnection {
    /// Start building a live telemetry connection.
    pub fn builder() -> LiveConnectionBuilder {
        LiveConnectionBuilder::default()
    }
}
