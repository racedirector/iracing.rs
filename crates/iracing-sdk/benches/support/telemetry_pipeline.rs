//! Deterministic provider and pipeline harness for delivery benchmarks.
//!
//! Source credits let a benchmark decide exactly when `Provider::next_frame`
//! may produce a frame. Each produced frame owns a fresh copy of the fixture
//! bytes, matching the production `FramePacket` ownership boundary. The latest
//! and on-demand case wrappers use the real delivery-policy implementations,
//! build subscriptions before timing, and expose explicit shutdown so worker
//! tasks do not leak between Criterion samples.

use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

use futures::{FutureExt, Stream, StreamExt, future::join_all};
use iracing_sdk::{
    FrameAdapter, FramePacket, Result, VariableSchema,
    benchmarking::{LatestPipeline, OnDemandPipeline},
    provider::Provider,
};
use tokio::sync::mpsc;

/// Type-erased subscription retained by a multi-subscriber benchmark case.
pub type BoxSubscription<T> = std::pin::Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

/// Control handle for releasing source frames and verifying provider activity.
pub struct SourceControl {
    credits: mpsc::UnboundedSender<()>,
    pub reads: Arc<AtomicUsize>,
    pub session_fetches: Arc<AtomicUsize>,
}

impl SourceControl {
    /// Permit the deterministic provider to produce one frame.
    pub fn release(&self) {
        self.credits
            .send(())
            .expect("deterministic provider should accept a source credit");
    }
}

/// Per-tick provider timestamps used only by latency diagnostics.
pub struct SourceTimes {
    starts: Vec<OnceLock<Instant>>,
}

impl SourceTimes {
    fn new(capacity: usize) -> Self {
        Self {
            starts: (0..capacity).map(|_| OnceLock::new()).collect(),
        }
    }

    /// Return elapsed nanoseconds since the provider produced `tick`.
    pub fn elapsed_nanos(&self, tick: u32) -> u64 {
        self.starts[tick as usize]
            .get()
            .expect("provider should timestamp every delivered tick")
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX)
    }
}

struct DeterministicProvider {
    credits: mpsc::UnboundedReceiver<()>,
    data: Arc<Vec<u8>>,
    schema: Arc<VariableSchema>,
    next_tick: u32,
    reads: Arc<AtomicUsize>,
    session_fetches: Arc<AtomicUsize>,
    times: Option<Arc<SourceTimes>>,
}

#[async_trait::async_trait]
impl Provider for DeterministicProvider {
    async fn next_frame(&mut self) -> Result<Option<FramePacket>> {
        if self.credits.recv().await.is_none() {
            return Ok(None);
        }

        let tick = self.next_tick;
        self.next_tick = self
            .next_tick
            .checked_add(1)
            .expect("benchmark tick overflow");
        if let Some(times) = &self.times {
            times.starts[tick as usize]
                .set(Instant::now())
                .expect("a benchmark tick should be produced once");
        }
        self.reads.fetch_add(1, Ordering::Relaxed);

        Ok(Some(FramePacket::new(
            self.data.as_slice().to_vec(),
            tick,
            0,
            Arc::clone(&self.schema),
        )))
    }

    async fn session_yaml(&mut self, _version: u32) -> Result<Option<String>> {
        self.session_fetches.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    fn tick_rate(&self) -> f64 {
        60.0
    }
}

fn source(
    data: Arc<Vec<u8>>,
    schema: Arc<VariableSchema>,
    timestamp_capacity: Option<usize>,
) -> (
    DeterministicProvider,
    SourceControl,
    Option<Arc<SourceTimes>>,
) {
    let (credits, receiver) = mpsc::unbounded_channel();
    let reads = Arc::new(AtomicUsize::new(0));
    let session_fetches = Arc::new(AtomicUsize::new(0));
    let times = timestamp_capacity.map(|capacity| Arc::new(SourceTimes::new(capacity)));
    (
        DeterministicProvider {
            credits: receiver,
            data,
            schema,
            next_tick: 0,
            reads: Arc::clone(&reads),
            session_fetches: Arc::clone(&session_fetches),
            times: times.clone(),
        },
        SourceControl {
            credits,
            reads,
            session_fetches,
        },
        times,
    )
}

/// Prepared latest-value pipeline and its active subscriptions.
pub struct LatestCase<T> {
    pub pipeline: LatestPipeline,
    pub source: SourceControl,
    pub subscriptions: Vec<BoxSubscription<T>>,
    pub times: Option<Arc<SourceTimes>>,
}

impl<T> LatestCase<T>
where
    T: FrameAdapter + Send + 'static,
{
    /// Construct the pipeline and validate every subscription before timing.
    pub fn new(
        data: Arc<Vec<u8>>,
        schema: Arc<VariableSchema>,
        subscribers: usize,
        timestamp_capacity: Option<usize>,
    ) -> Self {
        let (provider, source, times) = source(data, Arc::clone(&schema), timestamp_capacity);
        let pipeline = LatestPipeline::spawn(provider, schema);
        let subscriptions = (0..subscribers)
            .map(|_| {
                pipeline
                    .subscribe::<T>()
                    .expect("latest adapter validation")
            })
            .collect();
        Self {
            pipeline,
            source,
            subscriptions,
            times,
        }
    }

    /// Release and consume one source frame at a time without coalescing.
    pub async fn consume_paced<F>(&mut self, frames: usize, mut observe: F)
    where
        F: FnMut(&T),
    {
        for expected_tick in 0..frames as u32 {
            self.source.release();
            while self.pipeline.current_tick() != Some(expected_tick) {
                tokio::task::yield_now().await;
            }
            let delivered = join_all(self.subscriptions.iter_mut().map(StreamExt::next)).await;
            for item in &delivered {
                observe(item.as_ref().expect("latest subscription ended early"));
            }
        }
    }

    /// Offer bursts and consume only each subscriber's final latest value.
    pub async fn consume_bursts<F>(&mut self, bursts: usize, burst_size: usize, mut observe: F)
    where
        F: FnMut(&T),
    {
        for burst in 0..bursts {
            for _ in 0..burst_size {
                self.source.release();
            }
            let final_tick = ((burst + 1) * burst_size - 1) as u32;
            while self.pipeline.current_tick() != Some(final_tick) {
                tokio::task::yield_now().await;
            }
            let delivered = join_all(self.subscriptions.iter_mut().map(StreamExt::next)).await;
            for item in &delivered {
                observe(item.as_ref().expect("latest subscription ended early"));
            }
        }
    }

    /// Verify session behavior and join the pipeline worker outside timing.
    pub async fn shutdown(self) {
        assert_eq!(self.source.session_fetches.load(Ordering::Relaxed), 0);
        self.pipeline.shutdown().await;
    }
}

/// Prepared acknowledged-delivery pipeline and its active subscriptions.
pub struct OnDemandCase<T> {
    pub pipeline: OnDemandPipeline,
    pub source: SourceControl,
    pub subscriptions: Vec<BoxSubscription<T>>,
    pub times: Option<Arc<SourceTimes>>,
}

impl<T> OnDemandCase<T>
where
    T: FrameAdapter + Send + 'static,
{
    /// Construct and start the replay-style pipeline before timing.
    pub fn new(
        data: Arc<Vec<u8>>,
        schema: Arc<VariableSchema>,
        subscribers: usize,
        timestamp_capacity: Option<usize>,
    ) -> Self {
        let (provider, source, times) = source(data, Arc::clone(&schema), timestamp_capacity);
        let pipeline = OnDemandPipeline::spawn(provider, schema);
        let subscriptions = (0..subscribers)
            .map(|_| {
                pipeline
                    .subscribe::<T>()
                    .expect("replay adapter validation")
            })
            .collect();
        pipeline.start().expect("replay coordinator should start");
        Self {
            pipeline,
            source,
            subscriptions,
            times,
        }
    }

    /// Consume frames only after every subscriber acknowledges demand.
    pub async fn consume_acknowledged<F>(&mut self, frames: usize, mut observe: F)
    where
        F: FnMut(&T),
    {
        for _ in 0..frames {
            self.source.release();
            let delivered = join_all(self.subscriptions.iter_mut().map(StreamExt::next)).await;
            for item in &delivered {
                observe(item.as_ref().expect("replay subscription ended early"));
            }
        }
    }

    pub async fn consume_with_slow_ack<F>(&mut self, frames: usize, mut observe: F)
    where
        F: FnMut(&T),
    {
        if frames == 0 {
            return;
        }

        self.source.release();
        let first = join_all(self.subscriptions.iter_mut().map(StreamExt::next)).await;
        for item in &first {
            observe(item.as_ref().expect("replay subscription ended early"));
        }

        for _ in 1..frames {
            self.source.release();
            let fast_count = self.subscriptions.len().saturating_sub(1);
            for subscription in &mut self.subscriptions[..fast_count] {
                assert!(
                    subscription.next().now_or_never().is_none(),
                    "a fast subscriber advanced before the designated slow acknowledgement"
                );
            }

            let delivered = join_all(self.subscriptions.iter_mut().map(StreamExt::next)).await;
            for item in &delivered {
                observe(item.as_ref().expect("replay subscription ended early"));
            }
        }
    }

    /// Verify session behavior and join the pipeline worker outside timing.
    pub async fn shutdown(self) {
        assert_eq!(self.source.session_fetches.load(Ordering::Relaxed), 0);
        self.pipeline.shutdown().await;
    }
}

/// Sort samples in place and select a nearest-rank percentile value.
pub fn percentile(sorted_samples: &mut [u64], percentile: f64) -> u64 {
    assert!(!sorted_samples.is_empty());
    sorted_samples.sort_unstable();
    let index = ((sorted_samples.len() - 1) as f64 * percentile).ceil() as usize;
    sorted_samples[index]
}
