//! IBT connection for disk telemetry.

mod builder;

pub use builder::{IbtConnectionBuilder, NoSource, PathSource, ProviderSource};

use futures::{Stream, StreamExt};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use tokio_util::sync::CancellationToken;

use crate::{
    FrameAdapter, FramePacket, Result, UpdateRate, VariableSchema, provider::Provider,
    providers::ibt::IbtProvider, schema::SessionInfo, stream::ThrottleExt, telemetry::Telemetry,
};

/// IBT connection for disk telemetry.
pub struct IbtConnection {
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

impl IbtConnection {
    /// Start building an IBT connection.
    pub fn builder() -> IbtConnectionBuilder<NoSource> {
        IbtConnectionBuilder::default()
    }

    async fn from_provider(provider: IbtProvider, first_frame_timeout: Duration) -> Result<Self> {
        let schema = provider.schema();
        let source_hz = provider.tick_rate();

        Self::from_provider_parts(provider, schema, source_hz, first_frame_timeout).await
    }

    async fn from_provider_parts<P>(
        provider: P,
        schema: Arc<VariableSchema>,
        source_hz: f64,
        first_frame_timeout: Duration,
    ) -> Result<Self>
    where
        P: Provider,
    {
        // Spawn telemetry channels task
        let channels = Telemetry::spawn_preserving_last_frame(provider);

        // Wait for first frame.
        let mut frame_rx = channels.frames.clone();
        let wait_result = if first_frame_timeout.is_zero() {
            None
        } else {
            Some(
                tokio::time::timeout(first_frame_timeout, async {
                    while frame_rx.borrow().is_none() {
                        if frame_rx.changed().await.is_err() {
                            break;
                        }
                    }
                })
                .await,
            )
        };

        if matches!(wait_result, Some(Err(_))) {
            tracing::warn!(
                timeout = ?first_frame_timeout,
                "Timeout waiting for first frame from IBT provider."
            );
        }

        tracing::info!("IBT connection opened ({}Hz)", source_hz);

        Ok(Self {
            frames: channels.frames,
            sessions: channels.sessions,
            schema,
            source_hz,
            cancel: channels.cancel,
        })
    }

    /// Get telemetry frames as a stream.
    pub fn subscribe<T>(&self, rate: UpdateRate) -> impl Stream<Item = T> + 'static
    where
        T: FrameAdapter + Send + 'static,
    {
        let validation = T::validate_schema(&self.schema).expect("Schema validation failed");

        let frames = WatchStream::new(self.frames.clone()).filter_map(|opt| async move { opt });

        let effective_rate = rate.normalize(self.source_hz);

        match effective_rate {
            UpdateRate::Native => frames
                .map(move |packet| T::adapt(&packet, &validation))
                .boxed(),
            UpdateRate::Max(hz) => {
                // Throttle to the requested hz, then adapt.
                let interval = Duration::from_secs_f64(1.0 / hz as f64);
                frames
                    .throttle(interval)
                    .map(move |packet| T::adapt(&packet, &validation))
                    .boxed()
            }
        }
    }

    /// Get session updates as a stream.
    pub fn session_updates(&self) -> impl Stream<Item = Arc<SessionInfo>> + 'static {
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
        // Cancel tasks on drop for clean shutdown
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DynamicFrame, IRacingSDKError};
    use futures::StreamExt;
    use std::{collections::HashMap, future::pending, time::Instant};
    use test_utils::require_smallest_ibt_fixture;

    fn fixture_with_frame_count(frame_count: usize) -> Result<Vec<u8>> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for connection tests");
        let mut data = std::fs::read(path).expect("fixture should be readable");
        let reader = crate::ibt::IbtReader::from_bytes(data.clone())?;
        assert!(frame_count <= reader.total_frames());

        let frames_to_remove = reader.total_frames() - frame_count;
        data.truncate(data.len() - frames_to_remove * reader.variables().frame_size);
        Ok(data)
    }

    fn empty_schema() -> Arc<VariableSchema> {
        Arc::new(
            VariableSchema::new(HashMap::new(), 0)
                .expect("an empty schema should be valid for lifecycle tests"),
        )
    }

    struct PendingProvider;

    #[async_trait::async_trait]
    impl Provider for PendingProvider {
        async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
            pending().await
        }

        async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
            Ok(None)
        }

        fn tick_rate(&self) -> f64 {
            60.0
        }
    }

    struct ErrorProvider;

    #[async_trait::async_trait]
    impl Provider for ErrorProvider {
        async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
            Err(IRacingSDKError::connection_failed("startup failed"))
        }

        async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
            Ok(None)
        }

        fn tick_rate(&self) -> f64 {
            60.0
        }
    }

    #[tokio::test]
    async fn one_frame_remains_available_when_subscribing_after_build() -> Result<()> {
        let reader = crate::ibt::IbtReader::from_bytes(fixture_with_frame_count(1)?)?;
        let provider = IbtProvider::from_reader(reader);
        let connection = IbtConnection::from_provider(provider, Duration::from_secs(1)).await?;

        let mut frames = Box::pin(connection.subscribe::<DynamicFrame>(UpdateRate::Native));
        let frame = tokio::time::timeout(Duration::from_millis(100), frames.next())
            .await
            .expect("the retained frame should be immediately available");

        assert!(frame.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn eof_before_first_frame_finishes_without_waiting_for_timeout() -> Result<()> {
        let reader = crate::ibt::IbtReader::from_bytes(fixture_with_frame_count(0)?)?;
        let provider = IbtProvider::from_reader(reader);
        let started_at = Instant::now();

        let connection = IbtConnection::from_provider(provider, Duration::from_secs(1)).await?;

        assert!(started_at.elapsed() < Duration::from_millis(250));
        let mut frames = Box::pin(connection.subscribe::<DynamicFrame>(UpdateRate::Native));
        assert!(frames.next().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn first_frame_timeout_bounds_a_pending_provider() -> Result<()> {
        let timeout = Duration::from_millis(20);
        let started_at = Instant::now();

        let connection =
            IbtConnection::from_provider_parts(PendingProvider, empty_schema(), 60.0, timeout)
                .await?;

        assert!(started_at.elapsed() >= timeout);
        assert!(started_at.elapsed() < Duration::from_millis(500));
        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn provider_error_before_first_frame_remains_bounded_by_timeout() -> Result<()> {
        let timeout = Duration::from_millis(20);
        let started_at = Instant::now();

        let connection =
            IbtConnection::from_provider_parts(ErrorProvider, empty_schema(), 60.0, timeout)
                .await?;

        assert!(started_at.elapsed() >= timeout);
        assert!(started_at.elapsed() < Duration::from_millis(500));
        drop(connection);
        Ok(())
    }
}
