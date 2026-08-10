//! Telemetry spawns and manages telemetry processing tasks.

pub(crate) mod builder;
pub(crate) mod delivery_policy;
pub(crate) mod session_policy;

use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

use crate::{
    FramePacket,
    provider::Provider,
    schema::SessionInfo,
    telemetry::{
        delivery_policy::{DeliveryPolicy, OnDemandDelivery, ReplayDemand},
        session_policy::{IbtSessionPolicy, SessionPolicy},
    },
};

pub(crate) use self::builder::TelemetryBuilder;

/// A struct containing the telemetry communication channels and cancellation
/// token.
pub struct TelemetryChannels<
    F = watch::Receiver<Option<Arc<FramePacket>>>,
    S = watch::Receiver<Option<Arc<SessionInfo>>>,
> {
    /// Telemetry frame receiver.
    pub frames: F,
    /// Session info receiver.
    pub sessions: S,
    /// Cancellation token for shutdown.
    pub cancel: CancellationToken,
}

/// Communication channels for an on-demand IBT telemetry task.
pub(crate) type IbtTelemetryChannels =
    TelemetryChannels<mpsc::Sender<ReplayDemand>, watch::Receiver<Option<Arc<SessionInfo>>>>;

/// `Telemetry` spawns and manages telemetry processing tasks.
///
/// Spawns a frame-read task that owns the `Provider` and detects session changes.
/// Live YAML parsing runs on a separate FIFO worker so typed deserialization
/// does not block frame acquisition.
pub struct Telemetry;

impl Telemetry {
    /// Start building a telemetry task for the given provider.
    pub(crate) fn builder<P>(provider: P) -> TelemetryBuilder<P>
    where
        P: Provider,
    {
        TelemetryBuilder::new(provider)
    }

    /// Spawn telemetry tasks for the given provider.
    ///
    /// Returns watch receivers for frames and sessions, and a cancellation token.
    pub fn spawn<P>(provider: P) -> TelemetryChannels
    where
        P: Provider,
    {
        Self::builder(provider).build()
    }

    /// Spawn telemetry tasks for an IBT provider.
    ///
    /// The file's immutable session information is fetched and parsed once
    /// before frame processing begins. The returned frame handle accepts
    /// [`ReplayDemand`] values; each demand authorizes exactly one provider read.
    pub(crate) fn spawn_ibt<P>(provider: P) -> IbtTelemetryChannels
    where
        P: Provider,
    {
        let (frame_tx, frame_rx) = mpsc::channel(1);
        let (session_tx, session_rx) = watch::channel(None);

        Self::builder(provider)
            .with_delivery_policy(OnDemandDelivery::new(frame_rx), frame_tx)
            .with_session_policy(IbtSessionPolicy::new(session_tx), session_rx)
            .build()
    }

    // Reads frames and detects session changes.
    async fn read_task<P, D, S>(
        mut provider: P,
        mut delivery: D,
        mut sessions: S,
        cancel: CancellationToken,
    ) where
        P: Provider,
        D: DeliveryPolicy,
        S: SessionPolicy<P>,
    {
        tracing::info!("Frame reader task started");

        // Async task state
        let mut frame_count = 0u64;
        let mut error_count = 0u32;

        const MAX_ERRORS: u32 = 10;

        if !sessions.initialize(&mut provider, &cancel).await {
            tracing::info!("Session initialization cancelled");
            sessions.end().await;
            return;
        }

        loop {
            // Obtain delivery permission from the policy.
            let Some(permit) = delivery.acquire(&cancel).await else {
                tracing::info!("Frame read cancelled!");
                break;
            };

            // Use select to allow cancellation during provider.next_frame()
            let result = tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Frame reader cancelled during read");
                    break;
                }
                result = provider.next_frame() => result,
            };

            match result {
                Ok(Some(packet)) => {
                    frame_count += 1;
                    error_count = 0;

                    tracing::trace!(
                        "Frame {}: tick={}, session_version={}",
                        frame_count,
                        packet.tick,
                        packet.session_version
                    );

                    // Update session observers
                    if !sessions.observe(&mut provider, &packet, &cancel).await {
                        break;
                    }

                    // Update frame delivery
                    if !delivery.deliver(permit, packet).await {
                        tracing::debug!("Frame receiver dropped. Shutting down.");
                        break;
                    }
                }
                Ok(None) => {
                    tracing::info!("Provider stream ended after {} frames", frame_count);
                    // Update consumers that stream ended.
                    delivery.end(permit).await;
                    break;
                }
                Err(e) => {
                    // Provider error
                    error_count += 1;
                    tracing::error!("Provider error ({}/{}): {}", error_count, MAX_ERRORS, e);

                    if error_count >= MAX_ERRORS {
                        tracing::error!("Too many provider errors, shutting down!");
                        // Update consumers that stream ended.
                        delivery.end(permit).await;
                        break;
                    }

                    if !delivery.error(permit, e).await {
                        break;
                    }

                    // Exponential backoff before next loop.
                    let backoff = std::time::Duration::from_millis(50 * (1 << error_count.min(5)));
                    tokio::time::sleep(backoff).await;
                }
            }
        }

        // Finalize session state exactly once for every loop exit, including
        // cancellation and dropped delivery receivers.
        sessions.end().await;

        // When the loop breaks, log the number of processed frames.
        tracing::info!("Frame reader task ended (processed {} frames)", frame_count);
    }
}

#[cfg(test)]
mod tests {
    //! Characterization tests for the telemetry reader task.
    //!
    //! These tests use two deliberately different provider mocks:
    //!
    //! - [`FiniteProvider`] behaves like the current IBT provider. Every call to
    //!   `next_frame` completes immediately, so the on-demand delivery policy
    //!   must prevent the task from advancing without a matching replay demand.
    //! - [`LiveLikeProvider`] behaves like the live provider. Its
    //!   `next_frame` call waits for the test to supply another frame through a
    //!   channel, just as `LiveProvider` normally waits for another
    //!   shared-memory update. These tests protect the existing latest-wins
    //!   live behavior while delivery policies are introduced.
    //!
    //! Neither mock reads an IBT file or Windows shared memory. They isolate the
    //! scheduling and delivery behavior of `Telemetry::read_task`, which is the
    //! behavior under test here.

    use std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use tokio::sync::{mpsc, oneshot, watch};
    use tokio_util::sync::CancellationToken;

    use crate::{FramePacket, Result, VariableSchema, provider::Provider};

    use super::{
        ReplayDemand, Telemetry, TelemetryChannels, delivery_policy::LatestDelivery,
        session_policy::SessionPolicy,
    };

    /// A session policy that records how many times the telemetry task finalizes it.
    struct CountingSessionPolicy {
        end_count: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl<P: Provider> SessionPolicy<P> for CountingSessionPolicy {
        async fn observe(
            &mut self,
            _provider: &mut P,
            _frame: &FramePacket,
            _cancel: &CancellationToken,
        ) -> bool {
            true
        }

        async fn end(&mut self) {
            self.end_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Construct the smallest valid packet needed by the telemetry task.
    ///
    /// No telemetry fields are decoded in these tests, so an empty schema and
    /// payload keep the fixture focused on tick and session-version delivery.
    fn live_frame(tick: u32, session_version: u32) -> FramePacket {
        FramePacket::new(
            Vec::new(),
            tick,
            session_version,
            Arc::new(
                VariableSchema::new(HashMap::new(), 0)
                    .expect("an empty telemetry schema should be valid"),
            ),
        )
    }

    /// An eager, finite provider used to reproduce recorded-telemetry behavior.
    ///
    /// There are intentionally no waits or yields in `next_frame`. The delivery
    /// policy, rather than the provider, must therefore enforce replay pacing.
    struct FiniteProvider {
        next_tick: u32,
        frame_count: u32,
        session_yaml: Option<&'static str>,
        reads: mpsc::UnboundedSender<Option<u32>>,
    }

    impl FiniteProvider {
        fn new(
            frame_count: u32,
            session_yaml: Option<&'static str>,
        ) -> (Self, mpsc::UnboundedReceiver<Option<u32>>) {
            let (reads, observed_reads) = mpsc::unbounded_channel();

            (
                Self {
                    next_tick: 0,
                    frame_count,
                    session_yaml,
                    reads,
                },
                observed_reads,
            )
        }
    }

    #[async_trait::async_trait]
    impl Provider for FiniteProvider {
        async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
            if self.next_tick >= self.frame_count {
                let _ = self.reads.send(None);
                return Ok(None);
            }

            let tick = self.next_tick;
            self.next_tick += 1;
            let _ = self.reads.send(Some(tick));

            Ok(Some(live_frame(tick, 0)))
        }

        async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
            Ok(self.session_yaml.map(str::to_owned))
        }

        fn tick_rate(&self) -> f64 {
            60.0
        }
    }

    /// Ask the IBT frame channel to perform one provider read.
    ///
    /// Unlike the live watch channel, the IBT frame handle is a request sender.
    /// The one-shot response pairs this demand with exactly one frame, EOF, or
    /// provider error.
    async fn request_ibt_frame(frames: &mpsc::Sender<ReplayDemand>) -> Result<Option<FramePacket>> {
        let (response_tx, response_rx) = oneshot::channel();

        frames
            .send(ReplayDemand {
                response: response_tx,
            })
            .await
            .expect("telemetry should accept one replay demand");

        tokio::time::timeout(Duration::from_secs(1), response_rx)
            .await
            .expect("subscriber should not time out waiting for the replay response")
            .expect("telemetry should answer the replay demand")
    }

    #[tokio::test]
    async fn ibt_frame_channel_reads_one_ordered_frame_per_demand() {
        // Arrange: an IBT provider is finite and eager, but the telemetry task
        // must not touch it until the consumer requests a frame.
        let (provider, mut reads) = FiniteProvider::new(3, None);
        let channels = Telemetry::spawn_ibt(provider);

        assert!(
            tokio::time::timeout(Duration::from_millis(20), reads.recv())
                .await
                .is_err(),
            "the IBT cursor should remain parked while there is no demand"
        );

        // Act and assert: every request receives the next recorded frame. The
        // task parks again after each response instead of reading ahead.
        for expected_tick in 0..3 {
            let frame = request_ibt_frame(&channels.frames)
                .await
                .expect("the IBT provider read should succeed")
                .expect("a frame should remain in the recording");

            assert_eq!(frame.tick, expected_tick);
            assert_eq!(reads.recv().await, Some(Some(expected_tick)));
            assert!(
                tokio::time::timeout(Duration::from_millis(20), reads.recv())
                    .await
                    .is_err(),
                "one demand should authorize only one provider read"
            );
        }

        // EOF is also delivered in response to a demand; it never replaces a
        // frame that was already returned to the consumer.
        assert!(
            request_ibt_frame(&channels.frames)
                .await
                .expect("the EOF read should succeed")
                .is_none()
        );
        assert_eq!(reads.recv().await, Some(None));
    }

    #[tokio::test]
    async fn telemetry_finalizes_its_session_policy_once_at_eof() {
        let (provider, _reads) = FiniteProvider::new(0, None);
        let (frames, _frame_receiver) = watch::channel(None);
        let end_count = Arc::new(AtomicUsize::new(0));

        Telemetry::read_task(
            provider,
            LatestDelivery::new(frames),
            CountingSessionPolicy {
                end_count: Arc::clone(&end_count),
            },
            CancellationToken::new(),
        )
        .await;

        assert_eq!(
            end_count.load(Ordering::SeqCst),
            1,
            "the telemetry task should finalize a session policy exactly once"
        );
    }

    #[tokio::test]
    async fn ibt_session_channel_is_ready_before_frames_and_retained_after_eof() {
        // IBT session YAML describes the whole recording. It is published once
        // during startup, without requiring a frame demand.
        let (provider, _reads) = FiniteProvider::new(
            1,
            Some(include_str!(
                "../../../../test-data/session-yaml/profile_small.yaml"
            )),
        );
        let channels = Telemetry::spawn_ibt(provider);
        let mut sessions = channels.sessions.clone();

        tokio::time::timeout(Duration::from_secs(1), async {
            while sessions.borrow_and_update().is_none() {
                sessions
                    .changed()
                    .await
                    .expect("IBT session channel should remain connected");
            }
        })
        .await
        .expect("IBT session metadata should be published during startup");

        let session = sessions
            .borrow()
            .clone()
            .expect("the parsed IBT session should be available");

        assert!(
            request_ibt_frame(&channels.frames)
                .await
                .expect("the frame read should succeed")
                .is_some()
        );
        assert!(
            request_ibt_frame(&channels.frames)
                .await
                .expect("the EOF read should succeed")
                .is_none()
        );
        assert!(
            Arc::ptr_eq(
                &session,
                sessions
                    .borrow()
                    .as_ref()
                    .expect("EOF should retain the immutable IBT session")
            ),
            "IBT session metadata should remain the same snapshot after EOF"
        );
    }

    #[tokio::test]
    async fn ibt_cancellation_closes_the_pending_frame_channel() {
        // With no demand outstanding, cancellation wakes the parked telemetry
        // task and drops the receiving side of the request channel.
        let (provider, _reads) = FiniteProvider::new(1, None);
        let TelemetryChannels { frames, cancel, .. } = Telemetry::spawn_ibt(provider);

        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(1), frames.closed())
            .await
            .expect("cancellation should close the IBT frame request channel");
    }

    /// A controllable approximation of `LiveProvider`.
    ///
    /// The frame receiver models the shared-memory update wait: `next_frame`
    /// remains pending while the source is quiet and returns only after the
    /// test sends a frame. The two unbounded senders are observation hooks.
    /// They let tests verify provider calls without changing or reaching into
    /// the telemetry task itself.
    struct LiveLikeProvider {
        frames: mpsc::Receiver<FramePacket>,
        session_requests: mpsc::UnboundedSender<u32>,
        observed_ticks: mpsc::UnboundedSender<u32>,
    }

    #[async_trait::async_trait]
    impl Provider for LiveLikeProvider {
        async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
            // Keeping the sender alive with no queued frame leaves this await
            // pending, matching the normal "waiting for the next live update"
            // state of LiveProvider.
            let frame = self.frames.recv().await;
            if let Some(frame) = &frame {
                // Record that the provider handed this tick to the telemetry
                // task. This is not a second telemetry delivery channel.
                let _ = self.observed_ticks.send(frame.tick);
            }
            Ok(frame)
        }

        async fn session_yaml(&mut self, version: u32) -> Result<Option<String>> {
            // The mock records fetches but returns no YAML because these tests
            // characterize version-change detection, not YAML parsing.
            let _ = self.session_requests.send(version);
            Ok(None)
        }

        fn tick_rate(&self) -> f64 {
            60.0
        }
    }

    /// Wait until the live watch channel contains a frame.
    ///
    /// A watch receiver starts with `None`, representing a connected telemetry
    /// task that has not observed its first live frame yet. The helper ignores
    /// only that initial state and returns the first available snapshot.
    async fn receive_live_frame(
        frames: &mut watch::Receiver<Option<Arc<FramePacket>>>,
    ) -> Arc<FramePacket> {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(frame) = frames.borrow_and_update().clone() {
                    break frame;
                }

                frames
                    .changed()
                    .await
                    .expect("live frame sender should remain active");
            }
        })
        .await
        .expect("subscriber should receive a source-paced live frame")
    }

    /// Build a live-like provider and return its test controls.
    ///
    /// Return values, in order:
    ///
    /// 1. Frame input, standing in for iRacing shared-memory updates.
    /// 2. Session-version fetch observations.
    /// 3. Frame-read observations.
    /// 4. The provider passed to `Telemetry::spawn`.
    fn live_like_provider(
        capacity: usize,
    ) -> (
        mpsc::Sender<FramePacket>,
        mpsc::UnboundedReceiver<u32>,
        mpsc::UnboundedReceiver<u32>,
        LiveLikeProvider,
    ) {
        let (frame_tx, frame_rx) = mpsc::channel(capacity);
        let (session_request_tx, session_request_rx) = mpsc::unbounded_channel();
        let (observed_tick_tx, observed_tick_rx) = mpsc::unbounded_channel();

        (
            frame_tx,
            session_request_rx,
            observed_tick_rx,
            LiveLikeProvider {
                frames: frame_rx,
                session_requests: session_request_tx,
                observed_ticks: observed_tick_tx,
            },
        )
    }

    #[tokio::test]
    async fn live_context_delivers_the_first_source_paced_frame() {
        // Arrange: spawn telemetry with a provider that is initially waiting
        // for its first simulated shared-memory update.
        let (source, mut session_requests, _observed_ticks, provider) = live_like_provider(1);
        let channels = Telemetry::spawn(provider);
        let mut frames = channels.frames;

        // Act: make frame zero available to the provider.
        source
            .send(live_frame(0, 7))
            .await
            .expect("live-like source should remain connected");

        // Assert: the subscriber sees that first frame, and its previously
        // unseen session version triggers one session YAML request.
        let first = receive_live_frame(&mut frames).await;
        assert_eq!(first.tick, 0);
        assert_eq!(session_requests.recv().await, Some(7));

        channels.cancel.cancel();
    }

    #[tokio::test]
    async fn live_context_retains_the_current_frame_while_the_source_is_quiet() {
        // Arrange: publish and observe one frame while keeping the simulated
        // live source connected.
        let (source, _session_requests, _observed_ticks, provider) = live_like_provider(1);
        let channels = Telemetry::spawn(provider);
        let mut frames = channels.frames;

        source
            .send(live_frame(0, 0))
            .await
            .expect("live-like source should remain connected");
        let first = receive_live_frame(&mut frames).await;
        assert_eq!(first.tick, 0);

        // Act: publish nothing else. `next_frame` is now pending on the source
        // channel, which represents a quiet period between live updates.
        //
        // Assert: no update or EOF is emitted, and watch continues to retain
        // the most recently published live snapshot.
        assert!(
            tokio::time::timeout(Duration::from_millis(20), frames.changed())
                .await
                .is_err(),
            "a pending live provider should not end or publish another frame"
        );
        assert_eq!(
            frames
                .borrow()
                .as_ref()
                .expect("latest live frame should remain available")
                .tick,
            0
        );

        channels.cancel.cancel();
    }

    #[tokio::test]
    async fn live_context_fetches_session_yaml_once_per_version() {
        // Arrange: all frames come from one continuously connected live-like
        // provider. The observation channel records calls to session_yaml.
        let (source, mut session_requests, _observed_ticks, provider) = live_like_provider(1);
        let channels = Telemetry::spawn(provider);
        let mut frames = channels.frames;

        // The first frame introduces session version 3, so it must trigger a
        // fetch before the frame is published.
        source
            .send(live_frame(0, 3))
            .await
            .expect("live-like source should remain connected");
        assert_eq!(receive_live_frame(&mut frames).await.tick, 0);
        assert_eq!(session_requests.recv().await, Some(3));

        // A later frame with the same version must not fetch identical session
        // YAML again.
        source
            .send(live_frame(1, 3))
            .await
            .expect("live-like source should remain connected");
        frames
            .changed()
            .await
            .expect("second live frame should be published");
        assert_eq!(
            frames
                .borrow_and_update()
                .as_ref()
                .expect("second live frame should be available")
                .tick,
            1
        );
        assert!(
            session_requests.try_recv().is_err(),
            "an unchanged session version should not be fetched again"
        );

        // A newly observed version must trigger a new fetch while normal frame
        // delivery continues.
        source
            .send(live_frame(2, 4))
            .await
            .expect("live-like source should remain connected");
        frames
            .changed()
            .await
            .expect("third live frame should be published");
        assert_eq!(
            frames
                .borrow_and_update()
                .as_ref()
                .expect("third live frame should be available")
                .tick,
            2
        );
        assert_eq!(session_requests.recv().await, Some(4));

        channels.cancel.cancel();
    }

    #[tokio::test]
    async fn live_context_coalesces_a_burst_to_the_latest_frame() {
        const FRAME_COUNT: u32 = 48;

        // Arrange: provide enough source-channel capacity to queue a complete
        // burst without waiting for the watch subscriber.
        let (source, _session_requests, mut observed_ticks, provider) =
            live_like_provider(FRAME_COUNT as usize);
        let channels = Telemetry::spawn(provider);
        let mut frames = channels.frames;

        // Act: queue 48 source updates. This models a subscriber that is slower
        // than the live producer, not a lossless replay consumer.
        for tick in 0..FRAME_COUNT {
            source
                .send(live_frame(tick, 0))
                .await
                .expect("live-like source should remain connected");
        }

        // Wait until the telemetry task has read the complete burst. Without
        // this synchronization, checking watch could race with the producer
        // and assert on an arbitrary intermediate tick.
        for expected_tick in 0..FRAME_COUNT {
            assert_eq!(observed_ticks.recv().await, Some(expected_tick));
        }

        // Assert: watch eventually contains tick 47. Intermediate ticks may be
        // coalesced by design because live delivery represents current state,
        // not an event log.
        let latest = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(frame) = frames.borrow_and_update().clone()
                    && frame.tick == FRAME_COUNT - 1
                {
                    break frame;
                }

                frames
                    .changed()
                    .await
                    .expect("live frame sender should remain active");
            }
        })
        .await
        .expect("subscriber should observe the latest frame in the burst");

        assert_eq!(latest.tick, FRAME_COUNT - 1);
        channels.cancel.cancel();
    }
}
