use super::*;
use crate::telemetry::TelemetryChannels;
use delivery::ReplayDemand;
use std::time::Duration;
use tokio::sync::oneshot;
fn live_frame(tick: u32, session_version: u32) -> FramePacket {
    FramePacket::new(
        vec![0],
        tick,
        session_version,
        Arc::new(
            crate::test_utils::layout([], 1).expect("an empty telemetry layout should be valid"),
        ),
    )
    .unwrap()
}
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
    let channels = spawn_recorded_channels(provider);

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
async fn ibt_session_channel_is_ready_before_frames_and_retained_after_eof() {
    // IBT session YAML describes the whole recording. It is published once
    // during startup, without requiring a frame demand.
    let (provider, _reads) = FiniteProvider::new(
        1,
        Some(include_str!(
            "../../../../test-data/session-yaml/profile_small.yaml"
        )),
    );
    let channels = spawn_recorded_channels(provider);
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
    let TelemetryChannels { frames, cancel, .. } = spawn_recorded_channels(provider);

    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(1), frames.closed())
        .await
        .expect("cancellation should close the IBT frame request channel");
}
