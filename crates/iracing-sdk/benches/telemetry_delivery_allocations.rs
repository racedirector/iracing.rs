//! Deterministic telemetry delivery diagnostics.
//!
//! This executable reports allocation counts, provider-to-consumer latency
//! percentiles, and latest-value replacement behavior. It deliberately does
//! not publish Criterion timing estimates because allocator instrumentation and
//! timestamp sampling perturb the measured path.
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench telemetry-diagnostics
//! ```

mod support;

use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use support::{
    telemetry_pipeline::{LatestCase, OnDemandCase, percentile},
    workloads::TimedConsumerFrame47,
};

const FRAMES: usize = 1_024;
const LATENCY_FRAMES: usize = 2_048;
const BURSTS: usize = 64;
const BURST_SIZE: usize = 8;
const SUBSCRIBERS: [usize; 3] = [1, 4, 16];

struct CountingAllocator;

static ENABLED: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: The allocation request is forwarded unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: The pointer and layout originated from the system allocator.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: The allocation request is forwarded unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        // SAFETY: The pointer and layout originated from the system allocator.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn record(bytes: usize) {
    if ENABLED.load(Ordering::Relaxed) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    }
}

fn begin_counting() {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    ENABLED.store(true, Ordering::SeqCst);
}

fn end_counting() -> (u64, u64) {
    ENABLED.store(false, Ordering::SeqCst);
    (
        ALLOCATIONS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    )
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("telemetry diagnostic runtime should build")
}

fn allocation_diagnostics(
    runtime: &tokio::runtime::Runtime,
    data: &Arc<Vec<u8>>,
    schema: &Arc<iracing_sdk::VariableSchema>,
) {
    for subscribers in SUBSCRIBERS {
        let mut latest = {
            let _guard = runtime.enter();
            LatestCase::<TimedConsumerFrame47>::new(
                Arc::clone(data),
                Arc::clone(schema),
                subscribers,
                None,
            )
        };
        begin_counting();
        runtime.block_on(latest.consume_paced(FRAMES, |frame| {
            black_box(frame);
        }));
        let (allocations, bytes) = end_counting();
        let deliveries = (FRAMES * subscribers) as f64;
        println!(
            "telemetry allocations policy=latest-paced subscribers={subscribers} frames={FRAMES} allocations={allocations} bytes={bytes} allocations_per_frame={:.3} allocations_per_delivery={:.3} bytes_per_frame={:.1}",
            allocations as f64 / FRAMES as f64,
            allocations as f64 / deliveries,
            bytes as f64 / FRAMES as f64,
        );
        runtime.block_on(latest.shutdown());

        let mut replay = {
            let _guard = runtime.enter();
            OnDemandCase::<TimedConsumerFrame47>::new(
                Arc::clone(data),
                Arc::clone(schema),
                subscribers,
                None,
            )
        };
        begin_counting();
        runtime.block_on(replay.consume_acknowledged(FRAMES, |frame| {
            black_box(frame);
        }));
        let (allocations, bytes) = end_counting();
        println!(
            "telemetry allocations policy=ondemand-acknowledged subscribers={subscribers} frames={FRAMES} allocations={allocations} bytes={bytes} allocations_per_frame={:.3} allocations_per_delivery={:.3} bytes_per_frame={:.1}",
            allocations as f64 / FRAMES as f64,
            allocations as f64 / deliveries,
            bytes as f64 / FRAMES as f64,
        );
        runtime.block_on(replay.shutdown());
    }
}

fn print_latency(policy: &str, subscribers: usize, samples: &mut [u64]) {
    let p50 = percentile(samples, 0.50);
    let p95 = percentile(samples, 0.95);
    let p99 = percentile(samples, 0.99);
    println!(
        "telemetry latency policy={policy} subscribers={subscribers} samples={} p50_ns={p50} p95_ns={p95} p99_ns={p99}",
        samples.len()
    );
}

fn latency_diagnostics(
    runtime: &tokio::runtime::Runtime,
    data: &Arc<Vec<u8>>,
    schema: &Arc<iracing_sdk::VariableSchema>,
) {
    for subscribers in SUBSCRIBERS {
        let mut latest = {
            let _guard = runtime.enter();
            LatestCase::<TimedConsumerFrame47>::new(
                Arc::clone(data),
                Arc::clone(schema),
                subscribers,
                Some(LATENCY_FRAMES),
            )
        };
        let times = Arc::clone(latest.times.as_ref().expect("latest timestamps"));
        let mut samples = Vec::with_capacity(LATENCY_FRAMES * subscribers);
        runtime.block_on(latest.consume_paced(LATENCY_FRAMES, |frame| {
            black_box(&frame.frame_marker());
            samples.push(times.elapsed_nanos(frame.tick));
        }));
        print_latency("latest-paced", subscribers, &mut samples);
        runtime.block_on(latest.shutdown());

        let mut replay = {
            let _guard = runtime.enter();
            OnDemandCase::<TimedConsumerFrame47>::new(
                Arc::clone(data),
                Arc::clone(schema),
                subscribers,
                Some(LATENCY_FRAMES),
            )
        };
        let times = Arc::clone(replay.times.as_ref().expect("replay timestamps"));
        let mut samples = Vec::with_capacity(LATENCY_FRAMES * subscribers);
        runtime.block_on(replay.consume_acknowledged(LATENCY_FRAMES, |frame| {
            black_box(&frame.frame_marker());
            samples.push(times.elapsed_nanos(frame.tick));
        }));
        print_latency("ondemand-acknowledged", subscribers, &mut samples);
        runtime.block_on(replay.shutdown());
    }
}

fn delivery_diagnostics(
    runtime: &tokio::runtime::Runtime,
    data: &Arc<Vec<u8>>,
    schema: &Arc<iracing_sdk::VariableSchema>,
) {
    for subscribers in SUBSCRIBERS {
        let mut latest = {
            let _guard = runtime.enter();
            LatestCase::<TimedConsumerFrame47>::new(
                Arc::clone(data),
                Arc::clone(schema),
                subscribers,
                None,
            )
        };
        let mut delivered = 0usize;
        runtime.block_on(latest.consume_bursts(BURSTS, BURST_SIZE, |_| delivered += 1));
        let produced = latest.source.reads.load(Ordering::Relaxed);
        println!(
            "telemetry delivery policy=latest-burst-8 subscribers={subscribers} produced={produced} delivered={delivered} replaced={}",
            produced * subscribers - delivered
        );
        runtime.block_on(latest.shutdown());

        let mut replay = {
            let _guard = runtime.enter();
            OnDemandCase::<TimedConsumerFrame47>::new(
                Arc::clone(data),
                Arc::clone(schema),
                subscribers,
                None,
            )
        };
        let frames = BURSTS * BURST_SIZE;
        let mut delivered = 0usize;
        runtime.block_on(replay.consume_acknowledged(frames, |_| delivered += 1));
        let produced = replay.source.reads.load(Ordering::Relaxed);
        println!(
            "telemetry delivery policy=ondemand-acknowledged subscribers={subscribers} produced={produced} delivered={delivered} blocked_ack_barriers={}",
            frames.saturating_sub(1)
        );
        runtime.block_on(replay.shutdown());
    }
}

fn main() {
    let fixture = support::full_frame_fixture();
    let data = Arc::new(fixture.data);
    let schema = fixture.schema;
    let runtime = runtime();

    allocation_diagnostics(&runtime, &data, &schema);
    latency_diagnostics(&runtime, &data, &schema);
    delivery_diagnostics(&runtime, &data, &schema);
}
