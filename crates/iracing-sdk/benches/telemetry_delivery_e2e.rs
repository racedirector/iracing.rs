//! Deterministic, cross-platform telemetry delivery-pipeline benchmarks.
//!
//! # What "end to end" means here
//!
//! This target measures the in-process path from a controlled benchmark
//! provider to adapted subscriber output:
//!
//! ```text
//! source credit
//!   -> deterministic Provider::next_frame
//!   -> FramePacket allocation and byte copy
//!   -> production delivery policy
//!   -> subscription
//!   -> 47-field typed adapter
//!   -> benchmark consumer
//! ```
//!
//! It is "end to end" only within that boundary. The provider uses the
//! deterministic live-schema fixture in memory; no IBT file or Windows shared
//! memory is read. Simulator pacing, operating-system transport, connection
//! establishment, session parsing, serialization, and application work are not
//! measured.
//!
//! # Workloads and timing boundaries
//!
//! All cases run on a current-thread Tokio runtime and compare 1, 4, and 16
//! subscribers. Fixture loading, runtime creation, adapter validation,
//! subscription construction, and pipeline shutdown occur outside each
//! reported duration. The timed section includes source-frame construction,
//! asynchronous coordination, delivery-policy behavior, typed adaptation, and
//! consumption of the resulting frames.
//!
//! - `throughput/latest_paced` releases one source frame at a time, waits for
//!   the latest-value pipeline to publish that tick, and consumes one adapted
//!   value per subscriber before releasing the next source frame.
//! - `coalescing/latest_burst_8` releases eight source frames together, waits
//!   for the eighth tick, and then consumes only the latest adapted value from
//!   each subscriber. The configured element throughput counts offered source
//!   frames multiplied by subscribers, not the smaller number of outputs that
//!   survive latest-value coalescing.
//! - `throughput/ondemand_acknowledged` releases one frame and waits for every
//!   subscriber to request and receive it before advancing to the next frame.
//! - `backpressure/ondemand_slow_ack` adds deterministic subscriber-side work
//!   before acknowledging each on-demand frame.
//!
//! `iter_custom` is used so case construction and orderly task shutdown can be
//! excluded explicitly. Assertions after timing verify the provider performed
//! the expected number of reads. Adapted outputs are passed to
//! [`std::hint::black_box`] to keep the work observable.
//!
//! # Reading results
//!
//! Use these measurements to compare production delivery policies and scaling
//! across subscriber counts under deterministic load. Do not interpret them as
//! live iRacing latency or compare them directly with isolated decoding
//! microbenchmarks: the measured boundaries and units differ.
//!
//! Run this target with:
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench telemetry-delivery
//! ```

mod support;

use std::{hint::black_box, sync::Arc, time::Instant};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use support::{
    telemetry_pipeline::{LatestCase, OnDemandCase},
    workloads::TimedConsumerFrame47,
};

const SUBSCRIBERS: [usize; 3] = [1, 4, 16];
const BURST_SIZE: usize = 8;

/// Build the single-threaded runtime used consistently by every workload.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime should build")
}

/// Measure latest-value delivery when the controlled source is paced by consumption.
fn bench_latest_paced(c: &mut Criterion) {
    let fixture = support::full_frame_fixture();
    let data = Arc::new(fixture.data);
    let schema = fixture.schema;
    let runtime = runtime();
    let mut group = c.benchmark_group("telemetry_e2e/throughput/latest_paced/47_fields");

    for subscribers in SUBSCRIBERS {
        group.throughput(Throughput::Elements(subscribers as u64));
        group.bench_function(format!("subscribers_{subscribers}"), |b| {
            b.iter_custom(|iterations| {
                let frames = usize::try_from(iterations).expect("Criterion iteration overflow");
                let mut case = {
                    let _runtime_guard = runtime.enter();
                    LatestCase::<TimedConsumerFrame47>::new(
                        Arc::clone(&data),
                        Arc::clone(&schema),
                        subscribers,
                        None,
                    )
                };

                let started = Instant::now();
                runtime.block_on(case.consume_paced(frames, |frame| {
                    black_box(frame);
                }));
                let elapsed = started.elapsed();

                assert_eq!(
                    case.source.reads.load(std::sync::atomic::Ordering::Relaxed),
                    frames
                );
                runtime.block_on(case.shutdown());
                elapsed
            });
        });
    }
    group.finish();
}

/// Measure latest-value coalescing when eight source frames are offered together.
fn bench_latest_burst(c: &mut Criterion) {
    let fixture = support::full_frame_fixture();
    let data = Arc::new(fixture.data);
    let schema = fixture.schema;
    let runtime = runtime();
    let mut group = c.benchmark_group("telemetry_e2e/coalescing/latest_burst_8/47_fields");

    for subscribers in SUBSCRIBERS {
        group.throughput(Throughput::Elements((subscribers * BURST_SIZE) as u64));
        group.bench_function(format!("subscribers_{subscribers}"), |b| {
            b.iter_custom(|iterations| {
                let bursts = usize::try_from(iterations).expect("Criterion iteration overflow");
                let mut case = {
                    let _runtime_guard = runtime.enter();
                    LatestCase::<TimedConsumerFrame47>::new(
                        Arc::clone(&data),
                        Arc::clone(&schema),
                        subscribers,
                        None,
                    )
                };

                let started = Instant::now();
                runtime.block_on(case.consume_bursts(bursts, BURST_SIZE, |frame| {
                    black_box(frame);
                }));
                let elapsed = started.elapsed();

                assert_eq!(
                    case.source.reads.load(std::sync::atomic::Ordering::Relaxed),
                    bursts * BURST_SIZE
                );
                runtime.block_on(case.shutdown());
                elapsed
            });
        });
    }
    group.finish();
}

/// Measure acknowledged delivery in which all subscribers gate source progress.
fn bench_on_demand(c: &mut Criterion) {
    let fixture = support::full_frame_fixture();
    let data = Arc::new(fixture.data);
    let schema = fixture.schema;
    let runtime = runtime();
    let mut group = c.benchmark_group("telemetry_e2e/throughput/ondemand_acknowledged/47_fields");

    for subscribers in SUBSCRIBERS {
        group.throughput(Throughput::Elements(subscribers as u64));
        group.bench_function(format!("subscribers_{subscribers}"), |b| {
            b.iter_custom(|iterations| {
                let frames = usize::try_from(iterations).expect("Criterion iteration overflow");
                let mut case = {
                    let _runtime_guard = runtime.enter();
                    OnDemandCase::<TimedConsumerFrame47>::new(
                        Arc::clone(&data),
                        Arc::clone(&schema),
                        subscribers,
                        None,
                    )
                };

                let started = Instant::now();
                runtime.block_on(case.consume_acknowledged(frames, |frame| {
                    black_box(frame);
                }));
                let elapsed = started.elapsed();

                assert_eq!(
                    case.source.reads.load(std::sync::atomic::Ordering::Relaxed),
                    frames
                );
                runtime.block_on(case.shutdown());
                elapsed
            });
        });
    }
    group.finish();
}

fn bench_on_demand_slow_ack(c: &mut Criterion) {
    let fixture = support::full_frame_fixture();
    let data = Arc::new(fixture.data);
    let schema = fixture.schema;
    let runtime = runtime();
    let mut group = c.benchmark_group("telemetry_e2e/backpressure/ondemand_slow_ack/47_fields");

    for subscribers in SUBSCRIBERS {
        group.throughput(Throughput::Elements(subscribers as u64));
        group.bench_function(format!("subscribers_{subscribers}"), |b| {
            b.iter_custom(|iterations| {
                let frames = usize::try_from(iterations).expect("Criterion iteration overflow");
                let mut case = {
                    let _runtime_guard = runtime.enter();
                    OnDemandCase::<TimedConsumerFrame47>::new(
                        Arc::clone(&data),
                        Arc::clone(&schema),
                        subscribers,
                        None,
                    )
                };

                let started = Instant::now();
                runtime.block_on(case.consume_with_slow_ack(frames, |frame| {
                    black_box(frame);
                }));
                let elapsed = started.elapsed();

                assert_eq!(
                    case.source.reads.load(std::sync::atomic::Ordering::Relaxed),
                    frames
                );
                runtime.block_on(case.shutdown());
                elapsed
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_latest_paced,
    bench_latest_burst,
    bench_on_demand,
    bench_on_demand_slow_ack
);
criterion_main!(benches);
