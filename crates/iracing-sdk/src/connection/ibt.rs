//! IBT connection for IBT files

use futures::{Stream, StreamExt};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use tokio_util::sync::CancellationToken;

use crate::FramePacket;
use crate::UpdateRate;
use crate::emitter::TelemetryEmitter;
use crate::providers::IbtProvider;
use crate::providers::Provider;

use crate::{FrameAdapter, Result, SessionInfo, VariableSchema};

/// IBT connection from file
pub struct IbtConnection {
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

impl IbtConnection {
    /// Open an IBT file for replay.
    ///
    /// The replay stream is lazy: frames are read only after a subscriber polls
    /// the stream returned from [`Self::subscribe`].
    pub async fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        tracing::info!("Opening IBT file: {}", path.display());

        // Create provider and extract metadata
        let provider = IbtProvider::from_path(path)?;

        Self::with_provider(provider).await
    }

    /// Creates an IBT connection with a given provider.
    pub async fn with_provider(provider: IbtProvider) -> Result<Self> {
        let schema = provider.schema();
        let source_hz = provider.tick_rate();

        let channels = TelemetryEmitter::spawn(provider);

        tracing::info!("IBT connection opened ({}Hz)", source_hz);

        Ok(Self {
            frames: channels.frames,
            sessions: channels.sessions,
            schema,
            source_hz,
            cancel: channels.cancel,
        })
    }

    /// Subscribe to telemetry frames.
    ///
    /// IBT replay is a finite one-shot stream. The first subscriber consumes the
    /// replay from disk as the stream is polled; additional subscriptions panic.
    pub fn subscribe<T>(&self, rate: UpdateRate) -> impl Stream<Item = T> + 'static
    where
        T: FrameAdapter + Send + 'static,
    {
        // Validate schema once at subscription time
        let validation = T::validate_schema(&self.schema).expect("Schema validation failed");

        // Create base frame stream from watch channel
        let frames = WatchStream::new(self.frames.clone()).filter_map(|opt| async move { opt });

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
    pub fn session_updates(&self) -> impl Stream<Item = Arc<SessionInfo>> + 'static {
        // Simply watch the session channel - Driver handles all the complexity!
        WatchStream::new(self.sessions.clone()).filter_map(|opt| async move { opt })
    }

    /// Get current session info (if available)
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

impl Drop for IbtConnection {
    fn drop(&mut self) {
        tracing::debug!("Dropping IBT connection");
    }
}
