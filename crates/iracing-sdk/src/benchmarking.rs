//! Internal, feature-gated access to the production telemetry pipeline.
//!
//! This module is public only so external Criterion targets can drive private
//! delivery policies without widening the normal SDK API.

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
    AdapterValidation, FrameAdapter, FramePacket, Result, VariableSchema,
    connections::ibt::{
        coordinator::{self, ReplayControl},
        subscription::IbtSubscription,
    },
    provider::Provider,
    telemetry::{
        Telemetry,
        delivery_policy::{LatestDelivery, OnDemandDelivery},
        session_policy::SessionPolicy,
    },
};

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
    schema: Arc<VariableSchema>,
    cancel: CancellationToken,
    task: Option<JoinHandle<()>>,
}

impl LatestPipeline {
    /// Spawn the production telemetry loop with latest-value delivery.
    pub fn spawn<P>(provider: P, schema: Arc<VariableSchema>) -> Self
    where
        P: Provider,
    {
        let (sender, frames) = watch::channel(None);
        let (channels, task) = Telemetry::builder(provider)
            .with_delivery_policy(LatestDelivery::new(sender), frames)
            .with_session_policy(NoSessions::<P>::default(), ())
            .build_with_task();

        Self {
            frames: channels.frames,
            schema,
            cancel: channels.cancel,
            task: Some(task),
        }
    }

    /// Subscribe with the same initial/terminal watch semantics as a native live subscription.
    pub fn subscribe<T>(&self) -> Result<Pin<Box<dyn Stream<Item = T> + Send + 'static>>>
    where
        T: FrameAdapter + Send + 'static,
    {
        let validation = T::validate_schema(&self.schema)?;
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
    controls: mpsc::UnboundedSender<ReplayControl>,
    next_subscriber_id: AtomicU64,
    schema: Arc<VariableSchema>,
    cancel: CancellationToken,
    telemetry_task: Option<JoinHandle<()>>,
    coordinator_task: Option<JoinHandle<()>>,
}

impl OnDemandPipeline {
    /// Spawn the production telemetry loop and replay acknowledgement coordinator.
    pub fn spawn<P>(provider: P, schema: Arc<VariableSchema>) -> Self
    where
        P: Provider,
    {
        let (requests, request_receiver) = mpsc::channel(1);
        let (channels, telemetry_task) = Telemetry::builder(provider)
            .with_delivery_policy(OnDemandDelivery::new(request_receiver), requests)
            .with_session_policy(NoSessions::<P>::default(), ())
            .build_with_task();
        let (frames, controls, coordinator_task) =
            coordinator::spawn(channels.frames, channels.cancel.clone());

        Self {
            frames,
            controls,
            next_subscriber_id: AtomicU64::new(0),
            schema,
            cancel: channels.cancel,
            telemetry_task: Some(telemetry_task),
            coordinator_task: Some(coordinator_task),
        }
    }

    /// Create a coordinated subscription whose next poll acknowledges its prior frame.
    pub fn subscribe<T>(&self) -> Result<Pin<Box<dyn Stream<Item = T> + Send + 'static>>>
    where
        T: FrameAdapter + Send + 'static,
    {
        let validation: AdapterValidation = T::validate_schema(&self.schema)?;
        let subscriber_id = self.next_subscriber_id.fetch_add(1, Ordering::Relaxed);
        let _ = self.controls.send(ReplayControl::Join { subscriber_id });
        Ok(Box::pin(IbtSubscription::<T>::new(
            subscriber_id,
            self.frames.clone(),
            self.controls.clone(),
            validation,
        )))
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
    use std::{collections::HashMap, sync::atomic::AtomicUsize};

    use futures::{StreamExt, future::join_all};

    use super::*;
    use crate::DynamicFrame;

    struct ControlledProvider {
        credits: mpsc::UnboundedReceiver<()>,
        next_tick: u32,
        reads: Arc<AtomicUsize>,
        schema: Arc<VariableSchema>,
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
            Ok(Some(FramePacket::new(
                Vec::new(),
                tick,
                0,
                Arc::clone(&self.schema),
            )))
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
        Arc<VariableSchema>,
    ) {
        let (credits, receiver) = mpsc::unbounded_channel();
        let reads = Arc::new(AtomicUsize::new(0));
        let schema =
            Arc::new(VariableSchema::new(HashMap::new(), 0).expect("empty schema should validate"));
        (
            ControlledProvider {
                credits: receiver,
                next_tick: 0,
                reads: Arc::clone(&reads),
                schema: Arc::clone(&schema),
            },
            credits,
            reads,
            schema,
        )
    }

    #[tokio::test]
    async fn latest_pipeline_coalesces_a_deterministic_burst() {
        let (provider, credits, reads, schema) = source();
        let pipeline = LatestPipeline::spawn(provider, schema);
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
        let (provider, credits, reads, schema) = source();
        let pipeline = OnDemandPipeline::spawn(provider, schema);
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
