//! Internal, feature-gated delivery experiments and their behavioral harnesses.
//!
//! This module is public only so external Criterion targets can drive private
//! delivery policies without widening the normal SDK API.
//! Historical recorded coordinator, acknowledgement, and session policies live
//! only here; completed recordings use IbtFile and IbtReplay.

#![doc(hidden)]

use std::{
    marker::PhantomData,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};
use tokio_stream::wrappers::WatchStream;
use tokio_util::sync::CancellationToken;

use crate::{
    AdapterValidation, FrameAdapter, FramePacket, LayoutProvider, Result, TelemetryLayout,
    provider::Provider,
    telemetry::{Telemetry, delivery_policy::LatestDelivery, session_policy::SessionPolicy},
};

#[cfg(test)]
mod connection_tests;
mod coordinator;
mod delivery;
mod provider;
#[cfg(test)]
mod scheduling_tests;
mod session;
mod subscription;

use crate::{schema::SessionInfo, telemetry::TelemetryChannels};
use coordinator::ReplayControl;
use delivery::{OnDemandDelivery, ReplayDemand};
pub use provider::RecordedProvider;
use session::RecordedSessionPolicy;
use subscription::AcknowledgedSubscription;

type RecordedChannels =
    TelemetryChannels<mpsc::Sender<ReplayDemand>, watch::Receiver<Option<Arc<SessionInfo>>>>;

// Keep historical session fetch and parse on the telemetry task before its first frame.
#[cfg(test)]
fn spawn_recorded_channels<P: Provider>(provider: P) -> RecordedChannels {
    recorded_channels_with_task(provider).0
}

fn recorded_channels_with_task<P: Provider>(provider: P) -> (RecordedChannels, JoinHandle<()>) {
    let (requests, receiver) = mpsc::channel(1);
    let (sessions, receiver_sessions) = watch::channel(None);
    Telemetry::builder(provider)
        .with_delivery_policy(OnDemandDelivery::new(receiver), requests)
        .with_session_policy(RecordedSessionPolicy::new(sessions), receiver_sessions)
        .build_with_task()
}

/// A session policy that keeps session parsing out of delivery benchmarks.
struct NoSessions<P>(PhantomData<fn() -> P>);

impl<P> Default for NoSessions<P> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

#[async_trait]
impl<P: Provider> SessionPolicy<P> for NoSessions<P> {
    async fn observe(
        &mut self,
        _provider: &mut P,
        _frame: &FramePacket,
        _cancel: &CancellationToken,
    ) -> bool {
        true
    }

    async fn end(&mut self) {}
}

/// Cross-platform connection facade using production latest-value delivery.
pub struct LatestPipeline {
    frames: watch::Receiver<Option<Arc<FramePacket>>>,
    layout: Arc<TelemetryLayout>,
    cancel: CancellationToken,
    task: Option<JoinHandle<()>>,
}

impl LatestPipeline {
    /// Spawn the production telemetry loop with latest-value delivery.
    ///
    /// The pipeline retains the provider's exact shared layout allocation so
    /// adapter validation and produced packets cannot disagree on layout identity.
    pub fn spawn<P>(provider: P) -> Self
    where
        P: Provider + LayoutProvider,
    {
        let layout = Arc::clone(provider.layout());
        let (sender, frames) = watch::channel(None);
        let (channels, task) = Telemetry::builder(provider)
            .with_delivery_policy(LatestDelivery::new(sender), frames)
            .with_session_policy(NoSessions::<P>::default(), ())
            .build_with_task();

        Self {
            frames: channels.frames,
            layout,
            cancel: channels.cancel,
            task: Some(task),
        }
    }

    /// Subscribe with the same initial/terminal watch semantics as a native live subscription.
    pub fn subscribe<T>(&self) -> Result<Pin<Box<dyn Stream<Item = T> + Send + 'static>>>
    where
        T: FrameAdapter + Send + 'static,
    {
        let validation = T::validate_layout(&self.layout)?;
        let frames = WatchStream::new(self.frames.clone())
            .skip_while(|packet| std::future::ready(packet.is_none()))
            .take_while(|packet| std::future::ready(packet.is_some()))
            .filter_map(|packet| async move { packet })
            .map(move |packet| T::adapt(&packet, &validation));
        Ok(Box::pin(frames))
    }

    /// Return the currently retained source tick.
    pub fn current_tick(&self) -> Option<u32> {
        self.frames.borrow().as_ref().map(|packet| packet.tick)
    }

    /// Cancel and join the telemetry task.
    pub async fn shutdown(mut self) {
        self.cancel.cancel();
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

impl Drop for LatestPipeline {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// Cross-platform connection facade using acknowledged replay delivery.
pub struct OnDemandPipeline {
    frames: watch::Receiver<Option<Arc<FramePacket>>>,
    sessions: Option<watch::Receiver<Option<Arc<SessionInfo>>>>,
    controls: mpsc::UnboundedSender<ReplayControl>,
    next_subscriber_id: AtomicU64,
    layout: Arc<TelemetryLayout>,
    cancel: CancellationToken,
    telemetry_task: Option<JoinHandle<()>>,
    coordinator_task: Option<JoinHandle<()>>,
}

impl OnDemandPipeline {
    /// Spawn the shared telemetry loop with the experiment-owned acknowledgement coordinator.
    ///
    /// The pipeline retains the provider's exact shared layout allocation so
    /// adapter validation and produced packets cannot disagree on layout identity.
    pub fn spawn<P>(provider: P) -> Self
    where
        P: Provider + LayoutProvider,
    {
        let layout = Arc::clone(provider.layout());
        let (requests, request_receiver) = mpsc::channel(1);
        let (channels, telemetry_task) = Telemetry::builder(provider)
            .with_delivery_policy(OnDemandDelivery::new(request_receiver), requests)
            .with_session_policy(NoSessions::<P>::default(), ())
            .build_with_task();
        Self::from_channels(layout, channels, telemetry_task, None)
    }

    /// Preserve the recorded pipeline experiment, including immutable session parsing.
    /// Session initialization runs on the spawned task and stays inside timed draining.
    pub fn recorded<P: Provider + LayoutProvider>(provider: P) -> Self {
        let layout = Arc::clone(provider.layout());
        Self::recorded_with_layout(provider, layout)
    }

    fn recorded_with_layout<P: Provider>(provider: P, layout: Arc<TelemetryLayout>) -> Self {
        let (channels, telemetry_task) = recorded_channels_with_task(provider);
        let sessions = Some(channels.sessions.clone());
        Self::from_channels(layout, channels, telemetry_task, sessions)
    }

    fn from_channels<S>(
        layout: Arc<TelemetryLayout>,
        channels: TelemetryChannels<mpsc::Sender<ReplayDemand>, S>,
        telemetry_task: JoinHandle<()>,
        sessions: Option<watch::Receiver<Option<Arc<SessionInfo>>>>,
    ) -> Self {
        let (frames, controls, coordinator_task) =
            coordinator::spawn(channels.frames, channels.cancel.clone());
        Self {
            frames,
            sessions,
            controls,
            next_subscriber_id: AtomicU64::new(0),
            layout,
            cancel: channels.cancel,
            telemetry_task: Some(telemetry_task),
            coordinator_task: Some(coordinator_task),
        }
    }

    /// Return the retained session for the historical recorded workload.
    pub fn current_session(&self) -> Option<Arc<SessionInfo>> {
        self.sessions
            .as_ref()
            .and_then(|sessions| sessions.borrow().clone())
    }

    #[cfg(test)]
    fn current_frame(&self) -> Option<Arc<FramePacket>> {
        self.frames.borrow().clone()
    }

    /// Create the boxed subscription used by the generic delivery experiment.
    pub fn subscribe<T>(&self) -> Result<Pin<Box<dyn Stream<Item = T> + Send + 'static>>>
    where
        T: FrameAdapter + Send + 'static,
    {
        Ok(Box::pin(self.recorded_subscription::<T>()?))
    }

    /// Preserve the historical recorded experiment's single caller-owned stream box.
    pub fn recorded_subscription<T>(&self) -> Result<impl Stream<Item = T> + Send + 'static>
    where
        T: FrameAdapter + Send + 'static,
    {
        let validation: AdapterValidation = T::validate_layout(&self.layout)?;
        let subscriber_id = self.next_subscriber_id.fetch_add(1, Ordering::Relaxed);
        let _ = self.controls.send(ReplayControl::Join { subscriber_id });
        Ok(AcknowledgedSubscription::<T>::new(
            subscriber_id,
            self.frames.clone(),
            self.controls.clone(),
            validation,
        ))
    }

    /// Start coordinated replay after initial subscriptions have joined.
    pub fn start(&self) -> Result<()> {
        self.controls
            .send(ReplayControl::Start)
            .map_err(|_| crate::IRacingSDKError::connection_failed("replay coordinator stopped"))
    }

    /// Return the currently retained source tick.
    pub fn current_tick(&self) -> Option<u32> {
        self.frames.borrow().as_ref().map(|packet| packet.tick)
    }

    /// Cancel and join both pipeline tasks.
    pub async fn shutdown(mut self) {
        self.cancel.cancel();
        if let Some(task) = self.coordinator_task.take() {
            let _ = task.await;
        }
        if let Some(task) = self.telemetry_task.take() {
            let _ = task.await;
        }
    }
}

impl LayoutProvider for OnDemandPipeline {
    fn layout(&self) -> &Arc<TelemetryLayout> {
        &self.layout
    }
}

impl Drop for OnDemandPipeline {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(task) = self.coordinator_task.take() {
            task.abort();
        }
        if let Some(task) = self.telemetry_task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use futures::{StreamExt, future::join_all};

    use super::*;
    use crate::DynamicFrame;

    struct ControlledProvider {
        credits: mpsc::UnboundedReceiver<()>,
        next_tick: u32,
        reads: Arc<AtomicUsize>,
        layout: Arc<TelemetryLayout>,
    }

    impl LayoutProvider for ControlledProvider {
        fn layout(&self) -> &Arc<TelemetryLayout> {
            &self.layout
        }
    }

    #[async_trait]
    impl Provider for ControlledProvider {
        async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
            if self.credits.recv().await.is_none() {
                return Ok(None);
            }
            let tick = self.next_tick;
            self.next_tick += 1;
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(Some(
                FramePacket::new(vec![0], tick, 0, Arc::clone(&self.layout)).unwrap(),
            ))
        }

        async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
            panic!("delivery benchmarks must not fetch session YAML")
        }

        fn tick_rate(&self) -> f64 {
            60.0
        }
    }

    fn source() -> (
        ControlledProvider,
        mpsc::UnboundedSender<()>,
        Arc<AtomicUsize>,
    ) {
        let (credits, receiver) = mpsc::unbounded_channel();
        let reads = Arc::new(AtomicUsize::new(0));
        let layout =
            Arc::new(crate::test_utils::layout([], 1).expect("empty layout should validate"));
        (
            ControlledProvider {
                credits: receiver,
                next_tick: 0,
                reads: Arc::clone(&reads),
                layout,
            },
            credits,
            reads,
        )
    }

    #[tokio::test]
    async fn latest_pipeline_coalesces_a_deterministic_burst() {
        let (provider, credits, reads) = source();
        let pipeline = LatestPipeline::spawn(provider);
        let mut subscription = pipeline
            .subscribe::<DynamicFrame>()
            .expect("dynamic adapter should validate");

        for _ in 0..8 {
            credits.send(()).expect("provider should remain active");
        }
        while pipeline.current_tick() != Some(7) {
            tokio::task::yield_now().await;
        }

        assert_eq!(
            subscription
                .next()
                .await
                .expect("latest frame")
                .tick_count(),
            7
        );
        assert_eq!(reads.load(Ordering::SeqCst), 8);
        pipeline.shutdown().await;
    }

    #[tokio::test]
    async fn replay_waits_for_every_subscription_acknowledgement() {
        let (provider, credits, reads) = source();
        let pipeline = OnDemandPipeline::spawn(provider);
        let mut subscriptions: Vec<_> = (0..4)
            .map(|_| {
                pipeline
                    .subscribe::<DynamicFrame>()
                    .expect("dynamic adapter should validate")
            })
            .collect();
        pipeline.start().expect("replay should start");

        credits.send(()).expect("provider should remain active");
        let first = join_all(subscriptions.iter_mut().map(StreamExt::next)).await;
        assert!(
            first
                .into_iter()
                .all(|frame| frame.is_some_and(|frame| frame.tick_count() == 0))
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);

        credits.send(()).expect("provider should remain active");
        {
            let (fast, slow) = subscriptions.split_at_mut(3);
            let fast_next = join_all(fast.iter_mut().map(StreamExt::next));
            tokio::pin!(fast_next);
            tokio::select! {
                frames = &mut fast_next => panic!("fast subscribers advanced early: {frames:?}"),
                () = tokio::task::yield_now() => {}
            }
            for _ in 0..8 {
                tokio::task::yield_now().await;
            }
            assert_eq!(
                reads.load(Ordering::SeqCst),
                1,
                "the provider cursor advanced before the slow subscriber acknowledged"
            );

            let slow_next = slow[0].next();
            let (fast_frames, slow_frame) = tokio::join!(fast_next, slow_next);
            assert!(
                fast_frames
                    .into_iter()
                    .all(|frame| frame.is_some_and(|frame| frame.tick_count() == 1))
            );
            assert_eq!(slow_frame.expect("slow subscriber frame").tick_count(), 1);
        }
        assert_eq!(reads.load(Ordering::SeqCst), 2);

        drop(subscriptions);
        pipeline.shutdown().await;
    }
}
