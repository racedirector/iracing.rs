//! IBT connection for disk telemetry.

mod builder;
pub(crate) mod coordinator;
pub(crate) mod subscription;

pub use builder::{IbtConnectionBuilder, NoSource, PathSource, ProviderSource};

use futures::{Stream, StreamExt};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{mpsc, watch};
use tokio_stream::wrappers::WatchStream;
use tokio_util::sync::CancellationToken;

use crate::{
    FrameAdapter, FramePacket, IRacingSDKError, Result, SchemaProvider, VariableSchema,
    provider::Provider, providers::ibt::IbtProvider, schema::SessionInfo, telemetry::Telemetry,
};
use coordinator::ReplayControl;
use subscription::IbtSubscription;

/// IBT connection for disk telemetry.
pub struct IbtConnection {
    /// Frame receiver
    frames: watch::Receiver<Option<Arc<FramePacket>>>,

    /// Session receiver
    sessions: watch::Receiver<Option<Arc<SessionInfo>>>,

    /// Replay coordinator control channel
    controls: mpsc::UnboundedSender<ReplayControl>,

    /// Monotonic subscriber identifier allocator
    next_subscriber_id: AtomicU64,

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

    async fn from_provider(provider: IbtProvider) -> Result<Self> {
        let schema = provider.shared_schema();
        let source_hz = provider.tick_rate();

        Self::from_provider_parts(provider, schema, source_hz).await
    }

    async fn from_provider_parts<P>(
        provider: P,
        schema: Arc<VariableSchema>,
        source_hz: f64,
    ) -> Result<Self>
    where
        P: Provider,
    {
        // Spawn telemetry channels task
        let channels = Telemetry::spawn_ibt(provider);
        let (frames, controls, _coordinator_task) =
            coordinator::spawn(channels.frames, channels.cancel.clone());

        tracing::info!("IBT connection opened ({}Hz)", source_hz);

        Ok(Self {
            frames,
            sessions: channels.sessions,
            controls,
            next_subscriber_id: AtomicU64::new(0),
            schema,
            source_hz,
            cancel: channels.cancel,
        })
    }

    /// Subscribe to coordinated telemetry frames.
    ///
    /// Polling a subscription for its next item acknowledges the previously
    /// yielded frame. The shared cursor advances only after every current
    /// subscriber has acknowledged that frame.
    ///
    /// Create all initial subscriptions before calling [`Self::start`], and
    /// poll multiple subscriptions concurrently. Dropping a subscription
    /// removes it from the current acknowledgement barrier. If all
    /// subscriptions are dropped, replay pauses until another subscriber joins.
    pub fn subscribe<T>(&self) -> Result<impl Stream<Item = T> + 'static>
    where
        T: FrameAdapter + Send + 'static,
    {
        let validation = T::validate_schema(&self.schema)?;
        let subscriber_id = self.next_subscriber_id.fetch_add(1, Ordering::Relaxed);

        let _ = self.controls.send(ReplayControl::Join { subscriber_id });

        Ok(IbtSubscription::new(
            subscriber_id,
            self.frames.clone(),
            self.controls.clone(),
            validation,
        ))
    }

    /// Start coordinated frame delivery.
    ///
    /// Starting is idempotent. If no subscribers exist, the connection remains
    /// armed and requests its first frame when a subscriber joins.
    pub fn start(&self) -> Result<()> {
        self.controls
            .send(ReplayControl::Start)
            .map_err(|_| IRacingSDKError::connection_failed("IBT replay coordinator stopped"))
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

impl SchemaProvider for IbtConnection {
    fn schema(&self) -> &VariableSchema {
        self.schema.as_ref()
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
    use crate::test_utils::require_smallest_ibt_fixture;
    use crate::{DynamicFrame, IRacingSDKError};
    use futures::StreamExt;
    use std::{
        collections::HashMap,
        future::pending,
        time::{Duration, Instant},
    };
    use tokio::sync::mpsc;

    fn fixture_with_frame_count(frame_count: usize) -> Result<Vec<u8>> {
        let path = require_smallest_ibt_fixture()
            .expect("generated IBT fixture should be available for connection tests");
        let mut data = std::fs::read(path).expect("fixture should be readable");
        let reader = crate::ibt::IbtReader::from_bytes(data.clone())?;
        assert!(frame_count <= reader.total_frames());

        let frames_to_remove = reader.total_frames() - frame_count;
        data.truncate(data.len() - frames_to_remove * reader.schema().frame_size);
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

    struct TrackingProvider {
        next_tick: u32,
        frame_count: u32,
        reads: mpsc::UnboundedSender<Option<u32>>,
        schema: Arc<VariableSchema>,
    }

    struct ControlledProvider {
        frames: mpsc::Receiver<FramePacket>,
        reads: mpsc::UnboundedSender<()>,
    }

    #[async_trait::async_trait]
    impl Provider for ControlledProvider {
        async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
            let _ = self.reads.send(());
            Ok(self.frames.recv().await)
        }

        async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
            Ok(None)
        }

        fn tick_rate(&self) -> f64 {
            60.0
        }
    }

    #[async_trait::async_trait]
    impl Provider for TrackingProvider {
        async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
            if self.next_tick >= self.frame_count {
                let _ = self.reads.send(None);
                return Ok(None);
            }

            let tick = self.next_tick;
            self.next_tick += 1;
            let _ = self.reads.send(Some(tick));

            Ok(Some(FramePacket::new(
                Vec::new(),
                tick,
                0,
                Arc::clone(&self.schema),
            )))
        }

        async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
            Ok(None)
        }

        fn tick_rate(&self) -> f64 {
            60.0
        }
    }

    async fn tracking_connection(
        frame_count: u32,
    ) -> Result<(IbtConnection, mpsc::UnboundedReceiver<Option<u32>>)> {
        let schema = empty_schema();
        let (reads, observed_reads) = mpsc::unbounded_channel();
        let provider = TrackingProvider {
            next_tick: 0,
            frame_count,
            reads,
            schema: Arc::clone(&schema),
        };
        let connection = IbtConnection::from_provider_parts(provider, schema, 60.0).await?;

        Ok((connection, observed_reads))
    }

    #[tokio::test]
    async fn one_frame_is_delivered_after_start() -> Result<()> {
        let reader = crate::ibt::IbtReader::from_bytes(fixture_with_frame_count(1)?)?;
        let provider = IbtProvider::from_reader(reader);
        let connection = IbtConnection::from_provider(provider).await?;

        let mut frames = Box::pin(connection.subscribe::<DynamicFrame>()?);
        connection.start()?;
        let frame = tokio::time::timeout(Duration::from_millis(100), frames.next())
            .await
            .expect("the first demanded frame should be delivered");

        assert!(frame.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn eof_before_first_frame_returns_promptly() -> Result<()> {
        let reader = crate::ibt::IbtReader::from_bytes(fixture_with_frame_count(0)?)?;
        let provider = IbtProvider::from_reader(reader);
        let started_at = Instant::now();

        let connection = IbtConnection::from_provider(provider).await?;

        assert!(started_at.elapsed() < Duration::from_millis(250));
        let mut frames = Box::pin(connection.subscribe::<DynamicFrame>()?);
        connection.start()?;
        assert!(
            tokio::time::timeout(Duration::from_secs(1), frames.next())
                .await
                .expect("empty replay should report EOF")
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn connection_does_not_read_before_start() -> Result<()> {
        let (connection, mut reads) = tracking_connection(1).await?;
        let _frames = connection.subscribe::<DynamicFrame>()?;

        assert!(
            tokio::time::timeout(Duration::from_millis(20), reads.recv())
                .await
                .is_err(),
            "subscribing should not start the shared cursor"
        );
        Ok(())
    }

    #[tokio::test]
    async fn start_without_subscribers_arms_without_reading() -> Result<()> {
        let (connection, mut reads) = tracking_connection(1).await?;
        connection.start()?;

        assert!(
            tokio::time::timeout(Duration::from_millis(20), reads.recv())
                .await
                .is_err(),
            "a started connection should remain parked without subscribers"
        );
        Ok(())
    }

    #[tokio::test]
    async fn every_subscriber_acknowledges_before_the_cursor_advances() -> Result<()> {
        let (connection, mut reads) = tracking_connection(2).await?;
        let mut first = Box::pin(connection.subscribe::<DynamicFrame>()?);
        let mut second = Box::pin(connection.subscribe::<DynamicFrame>()?);
        connection.start()?;

        let (first_frame, second_frame) = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(first.next(), second.next())
        })
        .await
        .expect("both subscribers should receive the first frame");
        assert_eq!(
            first_frame
                .expect("first stream should be open")
                .tick_count(),
            0
        );
        assert_eq!(
            second_frame
                .expect("second stream should be open")
                .tick_count(),
            0
        );
        assert_eq!(reads.recv().await, Some(Some(0)));

        let mut first_next = Box::pin(first.next());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut first_next)
                .await
                .is_err(),
            "one acknowledgement should not advance the cursor"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(20), reads.recv())
                .await
                .is_err(),
            "the provider should remain parked for the second subscriber"
        );

        let (first_frame, second_frame) = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(first_next, second.next())
        })
        .await
        .expect("both acknowledgements should release the second frame");
        assert_eq!(
            first_frame
                .expect("first stream should be open")
                .tick_count(),
            1
        );
        assert_eq!(
            second_frame
                .expect("second stream should be open")
                .tick_count(),
            1
        );
        assert_eq!(reads.recv().await, Some(Some(1)));
        Ok(())
    }

    #[tokio::test]
    async fn subscriber_joining_midstream_joins_the_current_frame_barrier() -> Result<()> {
        let (connection, mut reads) = tracking_connection(2).await?;
        let mut first = Box::pin(connection.subscribe::<DynamicFrame>()?);
        connection.start()?;

        assert_eq!(
            first
                .next()
                .await
                .expect("first subscriber should receive frame zero")
                .tick_count(),
            0
        );
        assert_eq!(reads.recv().await, Some(Some(0)));

        let mut late = Box::pin(connection.subscribe::<DynamicFrame>()?);
        assert_eq!(
            late.next()
                .await
                .expect("late subscriber should receive the retained frame")
                .tick_count(),
            0
        );

        let mut first_next = Box::pin(first.next());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut first_next)
                .await
                .is_err(),
            "the original subscriber cannot advance without the late subscriber"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(20), reads.recv())
                .await
                .is_err(),
            "joining the current barrier should keep the provider parked"
        );

        let (first_frame, late_frame) = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(first_next, late.next())
        })
        .await
        .expect("both subscribers should release the next frame");
        assert_eq!(
            first_frame
                .expect("first subscriber should remain open")
                .tick_count(),
            1
        );
        assert_eq!(
            late_frame
                .expect("late subscriber should remain open")
                .tick_count(),
            1
        );
        assert_eq!(reads.recv().await, Some(Some(1)));
        Ok(())
    }

    #[tokio::test]
    async fn dropping_all_subscribers_pauses_and_resubscription_resumes() -> Result<()> {
        let (connection, mut reads) = tracking_connection(2).await?;
        let mut first = Box::pin(connection.subscribe::<DynamicFrame>()?);
        connection.start()?;

        let frame = tokio::time::timeout(Duration::from_secs(1), first.next())
            .await
            .expect("first subscriber should receive a frame")
            .expect("first subscriber should remain open");
        assert_eq!(frame.tick_count(), 0);
        assert_eq!(reads.recv().await, Some(Some(0)));

        drop(first);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), reads.recv())
                .await
                .is_err(),
            "dropping the last subscriber should park the cursor"
        );

        let mut resumed = Box::pin(connection.subscribe::<DynamicFrame>()?);
        let retained = tokio::time::timeout(Duration::from_secs(1), resumed.next())
            .await
            .expect("resubscribing should yield the retained frame")
            .expect("resubscribed stream should remain open");
        assert_eq!(retained.tick_count(), 0);

        let next = tokio::time::timeout(Duration::from_secs(1), resumed.next())
            .await
            .expect("acknowledging the retained frame should resume replay")
            .expect("another frame should remain");
        assert_eq!(next.tick_count(), 1);
        assert_eq!(reads.recv().await, Some(Some(1)));
        Ok(())
    }

    #[tokio::test]
    async fn replay_delivers_every_frame_and_retains_the_last_at_eof() -> Result<()> {
        let (connection, _reads) = tracking_connection(3).await?;
        let mut frames = Box::pin(connection.subscribe::<DynamicFrame>()?);
        connection.start()?;

        let mut ticks = Vec::new();
        tokio::time::timeout(Duration::from_secs(1), async {
            while let Some(frame) = frames.next().await {
                ticks.push(frame.tick_count());
            }
        })
        .await
        .expect("finite replay should reach EOF");

        assert_eq!(ticks, vec![0, 1, 2]);
        assert_eq!(
            connection
                .current_frame()
                .expect("EOF should retain the final frame")
                .tick,
            2
        );
        Ok(())
    }

    #[tokio::test]
    async fn dropping_all_subscribers_during_a_read_retains_its_response() -> Result<()> {
        let schema = empty_schema();
        let (source, source_frames) = mpsc::channel(1);
        let (reads, mut observed_reads) = mpsc::unbounded_channel();
        let provider = ControlledProvider {
            frames: source_frames,
            reads,
        };
        let connection =
            IbtConnection::from_provider_parts(provider, Arc::clone(&schema), 60.0).await?;
        let first = connection.subscribe::<DynamicFrame>();
        connection.start()?;

        tokio::time::timeout(Duration::from_secs(1), observed_reads.recv())
            .await
            .expect("the first subscriber should authorize a provider read")
            .expect("the provider should report its read");
        drop(first);

        source
            .send(FramePacket::new(Vec::new(), 0, 0, Arc::clone(&schema)))
            .await
            .expect("the in-flight provider read should remain connected");

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if connection
                    .current_frame()
                    .is_some_and(|frame| frame.tick == 0)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the in-flight response should be retained while paused");
        assert!(
            tokio::time::timeout(Duration::from_millis(20), observed_reads.recv())
                .await
                .is_err(),
            "retaining the response should not authorize another read"
        );

        let mut resumed = Box::pin(connection.subscribe::<DynamicFrame>()?);
        assert_eq!(
            resumed
                .next()
                .await
                .expect("resubscribing should yield the retained frame")
                .tick_count(),
            0
        );

        let mut next = Box::pin(resumed.next());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut next)
                .await
                .is_err(),
            "the resumed subscription should wait for the controlled provider"
        );
        tokio::time::timeout(Duration::from_secs(1), observed_reads.recv())
            .await
            .expect("acknowledging the retained frame should authorize another read")
            .expect("the provider should report its second read");

        source
            .send(FramePacket::new(Vec::new(), 1, 0, schema))
            .await
            .expect("the resumed provider read should remain connected");
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), next)
                .await
                .expect("the resumed subscriber should receive the next response")
                .expect("the resumed stream should remain open")
                .tick_count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn pending_provider_build_returns_promptly() -> Result<()> {
        let started_at = Instant::now();

        let connection =
            IbtConnection::from_provider_parts(PendingProvider, empty_schema(), 60.0).await?;

        assert!(started_at.elapsed() < Duration::from_millis(500));
        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn provider_error_before_first_frame_build_returns_promptly() -> Result<()> {
        let started_at = Instant::now();

        let connection =
            IbtConnection::from_provider_parts(ErrorProvider, empty_schema(), 60.0).await?;

        assert!(started_at.elapsed() < Duration::from_millis(500));
        drop(connection);
        Ok(())
    }
}
