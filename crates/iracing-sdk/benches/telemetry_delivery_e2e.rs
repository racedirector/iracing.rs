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
//! - `latency_diagnostics` separately records provider-timestamp-to-consumer
//!   elapsed time and prints p50, p95, and p99 values. Its Criterion-visible
//!   `completed` case is only a marker; it is not the latency measurement.
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
//! cargo bench -p iracing-sdk --features benchmark --bench telemetry_delivery_e2e
//! ```

mod support;

use std::{hint::black_box, sync::Arc, time::Instant};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use support::{
    telemetry_pipeline::{LatestCase, OnDemandCase, percentile},
    workloads::TimedConsumerFrame47,
};

const SUBSCRIBERS: [usize; 3] = [1, 4, 16];
const LATENCY_FRAMES: usize = 2_048;
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

/// Print percentile diagnostics calculated from explicit latency samples.
fn print_latency(label: &str, subscribers: usize, samples: &mut [u64]) {
    let p50 = percentile(samples, 0.50);
    let p95 = percentile(samples, 0.95);
    let p99 = percentile(samples, 0.99);
    println!(
        "telemetry_e2e latency policy={label} subscribers={subscribers} samples={} p50_ns={p50} p95_ns={p95} p99_ns={p99}",
        samples.len()
    );
}

/// Collect latency samples outside Criterion's throughput measurements.
fn latency_diagnostics(c: &mut Criterion) {
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
                Some(LATENCY_FRAMES),
            )
        };
        let latest_times = Arc::clone(latest.times.as_ref().expect("latest timestamps"));
        let mut latest_samples = Vec::with_capacity(LATENCY_FRAMES * subscribers);
        runtime.block_on(latest.consume_paced(LATENCY_FRAMES, |frame| {
            black_box(&frame.frame_marker());
            latest_samples.push(latest_times.elapsed_nanos(frame.tick));
        }));
        print_latency("latest_paced", subscribers, &mut latest_samples);
        runtime.block_on(latest.shutdown());

        let mut replay = {
            let _runtime_guard = runtime.enter();
            OnDemandCase::<TimedConsumerFrame47>::new(
                Arc::clone(&data),
                Arc::clone(&schema),
                subscribers,
                Some(LATENCY_FRAMES),
            )
        };
        let replay_times = Arc::clone(replay.times.as_ref().expect("replay timestamps"));
        let mut replay_samples = Vec::with_capacity(LATENCY_FRAMES * subscribers);
        runtime.block_on(replay.consume_acknowledged(LATENCY_FRAMES, |frame| {
            black_box(&frame.frame_marker());
            replay_samples.push(replay_times.elapsed_nanos(frame.tick));
        }));
        print_latency("ondemand_acknowledged", subscribers, &mut replay_samples);
        runtime.block_on(replay.shutdown());
    }

    // Keep a Criterion-visible marker for diagnostic execution without timing
    // timestamp collection as a throughput result.
    c.bench_function("telemetry_e2e/latency_diagnostics/completed", |b| {
        b.iter(|| black_box(()))
    });
}

fn delivery_diagnostics(c: &mut Criterion) {
    const BURSTS: usize = 64;
    const REPLAY_FRAMES: usize = BURSTS * BURST_SIZE;

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
        let mut latest_deliveries = 0_usize;
        runtime.block_on(latest.consume_bursts(BURSTS, BURST_SIZE, |_| {
            latest_deliveries += 1;
        }));
        let produced = latest
            .source
            .reads
            .load(std::sync::atomic::Ordering::Relaxed);
        let replaced = produced * subscribers - latest_deliveries;
        println!(
            "telemetry_e2e delivery policy=latest_burst_8 subscribers={subscribers} produced={produced} delivered={latest_deliveries} replaced={replaced} dropped=0 blocked=0"
        );
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
        let mut replay_deliveries = 0_usize;
        runtime.block_on(replay.consume_acknowledged(REPLAY_FRAMES, |_| {
            replay_deliveries += 1;
        }));
        let produced = replay
            .source
            .reads
            .load(std::sync::atomic::Ordering::Relaxed);
        println!(
            "telemetry_e2e delivery policy=ondemand_acknowledged subscribers={subscribers} produced={produced} delivered={replay_deliveries} replaced=0 dropped=0 blocked_ack_barriers={}",
            REPLAY_FRAMES.saturating_sub(1)
        );
        runtime.block_on(replay.shutdown());
    }

    c.bench_function("telemetry_e2e/delivery_diagnostics/completed", |b| {
        b.iter(|| black_box(()))
    });
}

criterion_group!(
    benches,
    delivery_diagnostics,
    latency_diagnostics,
    bench_latest_paced,
    bench_latest_burst,
    bench_on_demand,
    bench_on_demand_slow_ack
);
criterion_main!(benches);
