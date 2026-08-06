//! Allocation diagnostics for deterministic end-to-end telemetry delivery.

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
    telemetry_pipeline::{LatestCase, OnDemandCase},
    workloads::TimedConsumerFrame47,
};

const FRAMES: usize = 1_024;
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

fn report(policy: &str, subscribers: usize, allocations: u64, bytes: u64) {
    let deliveries = (FRAMES * subscribers) as f64;
    println!(
        "telemetry_e2e allocations policy={policy} subscribers={subscribers} frames={FRAMES} allocations={allocations} bytes={bytes} allocations_per_frame={:.3} allocations_per_delivery={:.3} bytes_per_frame={:.1}",
        allocations as f64 / FRAMES as f64,
        allocations as f64 / deliveries,
        bytes as f64 / FRAMES as f64,
    );
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("allocation diagnostic runtime should build")
}

fn main() {
    let fixture = support::full_frame_fixture();
    let data = Arc::new(fixture.data);
    let schema = fixture.schema;
    let runtime = runtime();

    for subscribers in SUBSCRIBERS {
        let mut latest = {
            let _runtime_guard = runtime.enter();
            LatestCase::<TimedConsumerFrame47>::new(
                Arc::clone(&data),
                Arc::clone(&schema),
                subscribers,
                None,
            )
        };
        begin_counting();
        runtime.block_on(latest.consume_paced(FRAMES, |frame| {
            black_box(frame);
        }));
        let (allocations, bytes) = end_counting();
        report("latest_paced", subscribers, allocations, bytes);
        runtime.block_on(latest.shutdown());

        let mut replay = {
            let _runtime_guard = runtime.enter();
            OnDemandCase::<TimedConsumerFrame47>::new(
                Arc::clone(&data),
                Arc::clone(&schema),
                subscribers,
                None,
            )
        };
        begin_counting();
        runtime.block_on(replay.consume_acknowledged(FRAMES, |frame| {
            black_box(frame);
        }));
        let (allocations, bytes) = end_counting();
        report("ondemand_acknowledged", subscribers, allocations, bytes);
        runtime.block_on(replay.shutdown());
    }
}
