use super::*;
use crate::test_utils::require_smallest_ibt_fixture;
use crate::{DynamicFrame, IRacingSDKError};
use futures::StreamExt;
use std::{
    future::pending,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

fn fixture_with_frame_count(frame_count: usize) -> Result<Vec<u8>> {
    let path = require_smallest_ibt_fixture()
        .expect("generated IBT fixture should be available for connection tests");
    let mut data = std::fs::read(path).expect("fixture should be readable");
    let reader = crate::ibt::IbtFile::from_bytes(data.clone())?;
    assert!(frame_count <= reader.physical_layout().frame_count());

    let frames_to_remove = reader.physical_layout().frame_count() - frame_count;
    data.truncate(data.len() - frames_to_remove * reader.physical_layout().frame_size());
    Ok(data)
}

fn empty_schema() -> Arc<TelemetryLayout> {
    Arc::new(
        crate::test_utils::layout([], 1)
            .expect("an empty layout should be valid for lifecycle tests"),
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
    layout: Arc<TelemetryLayout>,
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

        Ok(Some(
            FramePacket::new(vec![0], tick, 0, Arc::clone(&self.layout)).unwrap(),
        ))
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
) -> Result<(OnDemandPipeline, mpsc::UnboundedReceiver<Option<u32>>)> {
    let layout = empty_schema();
    let (reads, observed_reads) = mpsc::unbounded_channel();
    let provider = TrackingProvider {
        next_tick: 0,
        frame_count,
        reads,
        layout: Arc::clone(&layout),
    };
    let connection = from_provider_parts(provider, layout, 60.0).await?;

    Ok((connection, observed_reads))
}

#[tokio::test]
async fn configured_replay_delivers_only_remaining_bounded_frames() -> Result<()> {
    let file = crate::ibt::IbtFile::from_bytes(fixture_with_frame_count(4)?)?;
    let layout = Arc::clone(file.telemetry_layout());
    let mut replay = file.replay();
    replay.set_range(1..4)?;
    replay.seek(2)?;
    let connection = from_replay(replay).await?;
    assert!(Arc::ptr_eq(connection.layout(), &layout));
    let mut frames = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
    connection.start()?;
    let ticks = tokio::time::timeout(Duration::from_secs(1), async {
        let mut ticks = Vec::new();
        while let Some(frame) = frames.next().await {
            ticks.push(frame.tick_count());
        }
        ticks
    })
    .await
    .expect("bounded replay should reach EOF");
    assert_eq!(ticks, vec![2, 3]);
    let final_packet = connection.current_frame().unwrap();
    assert_eq!(final_packet.tick, 3);
    assert!(Arc::ptr_eq(final_packet.layout(), &layout));
    Ok(())
}

#[tokio::test]
async fn file_provider_connection_and_packet_share_one_layout() -> Result<()> {
    let file = crate::ibt::IbtFile::from_bytes(fixture_with_frame_count(1)?)?;
    let layout = Arc::clone(file.telemetry_layout());
    let provider = RecordedProvider::from_replay(file.replay());
    assert!(Arc::ptr_eq(provider.layout(), &layout));
    let connection = from_provider(provider).await?;
    assert!(Arc::ptr_eq(connection.layout(), &layout));
    let mut frames = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
    connection.start()?;
    assert!(
        tokio::time::timeout(Duration::from_secs(1), frames.next())
            .await
            .expect("file frame should be delivered")
            .is_some()
    );
    let packet = connection.current_frame().unwrap();
    assert!(Arc::ptr_eq(packet.layout(), &layout));
    Ok(())
}

#[tokio::test]
async fn one_frame_is_delivered_after_start() -> Result<()> {
    let reader = crate::ibt::IbtFile::from_bytes(fixture_with_frame_count(1)?)?;
    let provider = RecordedProvider::from_replay(reader.replay());
    let connection = from_provider(provider).await?;

    let mut frames = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
    connection.start()?;
    let frame = tokio::time::timeout(Duration::from_millis(100), frames.next())
        .await
        .expect("the first demanded frame should be delivered");

    assert!(frame.is_some());
    Ok(())
}

#[tokio::test]
async fn eof_before_first_frame_returns_promptly() -> Result<()> {
    let reader = crate::ibt::IbtFile::from_bytes(fixture_with_frame_count(0)?)?;
    let provider = RecordedProvider::from_replay(reader.replay());
    let started_at = Instant::now();

    let connection = from_provider(provider).await?;

    assert!(started_at.elapsed() < Duration::from_millis(250));
    let mut frames = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
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
    let _frames = connection.recorded_subscription::<DynamicFrame>()?;

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
    let mut first = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
    let mut second = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
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
    let mut first = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
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

    let mut late = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
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
    let mut first = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
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

    let mut resumed = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
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
    let mut frames = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
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
    let layout = empty_schema();
    let (source, source_frames) = mpsc::channel(1);
    let (reads, mut observed_reads) = mpsc::unbounded_channel();
    let provider = ControlledProvider {
        frames: source_frames,
        reads,
    };
    let connection = from_provider_parts(provider, Arc::clone(&layout), 60.0).await?;
    let first = connection.recorded_subscription::<DynamicFrame>();
    connection.start()?;

    tokio::time::timeout(Duration::from_secs(1), observed_reads.recv())
        .await
        .expect("the first subscriber should authorize a provider read")
        .expect("the provider should report its read");
    drop(first);

    source
        .send(FramePacket::new(vec![0], 0, 0, Arc::clone(&layout)).unwrap())
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

    let mut resumed = Box::pin(connection.recorded_subscription::<DynamicFrame>()?);
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
        .send(FramePacket::new(vec![0], 1, 0, layout).unwrap())
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

    let connection = from_provider_parts(PendingProvider, empty_schema(), 60.0).await?;

    assert!(started_at.elapsed() < Duration::from_millis(500));
    drop(connection);
    Ok(())
}

#[tokio::test]
async fn provider_error_before_first_frame_build_returns_promptly() -> Result<()> {
    let started_at = Instant::now();

    let connection = from_provider_parts(ErrorProvider, empty_schema(), 60.0).await?;

    assert!(started_at.elapsed() < Duration::from_millis(500));
    drop(connection);
    Ok(())
}

async fn from_provider_parts<P: Provider>(
    provider: P,
    layout: Arc<TelemetryLayout>,
    _source_hz: f64,
) -> Result<OnDemandPipeline> {
    Ok(OnDemandPipeline::recorded_with_layout(provider, layout))
}
async fn from_provider(provider: RecordedProvider) -> Result<OnDemandPipeline> {
    Ok(OnDemandPipeline::recorded(provider))
}
async fn from_replay(replay: crate::ibt::IbtReplay) -> Result<OnDemandPipeline> {
    from_provider(RecordedProvider::from_replay(replay)).await
}
