//! Performance baseline for the public [`IbtReader`] storage API.
//!
//! Sequential case names are retained across the reader/provider API migration.
//! The random-frame case uses direct indexed reads because the reader no longer
//! exposes cursor-only seeking; it is not comparable to that old experiment.
//!
//! Run only this focused target with:
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench ibt-reader-performance
//! ```
//!
//! # Timed boundaries
//!
//! - `ibt_reader_open` includes opening the path, reading whatever the reader
//!   implementation requires, parsing metadata, and dropping the reader.
//! - `ibt_reader_sequential_replay` constructs the reader outside the timed
//!   routine, then reads every owned frame through `frame(index)`.
//! - `ibt_provider_sequential_replay` constructs the replay provider outside
//!   the timed routine, then requests frames through `Provider::next_frame()`.
//! - `ibt_connection_sequential_replay` constructs the public disk connection
//!   and one dynamic-frame subscription outside the timed routine, then starts
//!   and drains the coordinated replay stream.
//! - `ibt_reader_random_frame_read` constructs the reader outside the timed routine,
//!   then performs 1,024 deterministic `frame(index)` calls, including frame I/O.
//!
//! Fixtures are sequentially prewarmed through plain file reads before each
//! timed case. Results are warm-cache local storage measurements, not cold-open
//! latency. Compare revisions on the same machine with the same fixtures and
//! build profile. These timing benchmarks do not measure retained heap; the
//! #84 measurement layer records that separately.

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use futures::StreamExt;
use iracing_sdk::{
    DynamicFrame, IbtConnection, ibt::IbtReader, provider::Provider, providers::ibt::IbtProvider,
};
use std::{hint::black_box, time::Duration, time::Instant};

mod support;
use support::ibt::Recording;

const RANDOM_SEEKS_PER_ITERATION: usize = 1_024;

fn recordings() -> [Recording; 2] {
    [
        Recording::load(
            "small_5_9mb",
            "test-data/fordmustanggt4_donington national 2026-04-09 21-57-46.ibt",
        ),
        Recording::load("large_142_6mb", "test-data/ibt/b_mustang_bristol_race.ibt"),
    ]
}

fn bench_open(c: &mut Criterion) {
    let mut group = c.benchmark_group("ibt_reader_open");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for recording in recordings() {
        recording.prewarm();
        group.throughput(Throughput::Bytes(recording.file_size));
        group.bench_with_input(
            BenchmarkId::from_parameter(recording.name),
            &recording.path,
            |b, path| {
                b.iter(|| {
                    let reader = IbtReader::open(black_box(path)).expect("fixture should open");
                    black_box(reader)
                });
            },
        );
    }

    group.finish();
}

fn bench_sequential_replay(c: &mut Criterion) {
    let mut group = c.benchmark_group("ibt_reader_sequential_replay");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for recording in recordings() {
        recording.prewarm();
        group.throughput(Throughput::Bytes(recording.replay_bytes));
        group.bench_with_input(
            BenchmarkId::from_parameter(recording.name),
            &recording.path,
            |b, path| {
                b.iter_batched(
                    || IbtReader::open(path).expect("fixture should open"),
                    |reader| {
                        for index in 0..reader.layout().frame_count() {
                            black_box(reader.frame(index).expect("fixture frame should read"));
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime should build")
}

fn bench_provider_sequential_replay(c: &mut Criterion) {
    let runtime = runtime();
    let mut group = c.benchmark_group("ibt_provider_sequential_replay");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for recording in recordings() {
        recording.prewarm();
        group.throughput(Throughput::Bytes(recording.replay_bytes));
        group.bench_with_input(
            BenchmarkId::from_parameter(recording.name),
            &recording.path,
            |b, path| {
                b.iter_batched(
                    || IbtProvider::open(path).expect("fixture provider should open"),
                    |mut provider| {
                        runtime.block_on(async {
                            while let Some(frame) = provider
                                .next_frame()
                                .await
                                .expect("fixture provider frame should read")
                            {
                                black_box(frame);
                            }
                        });
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_connection_sequential_replay(c: &mut Criterion) {
    let runtime = runtime();
    let mut group = c.benchmark_group("ibt_connection_sequential_replay");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for recording in recordings() {
        recording.prewarm();
        group.throughput(Throughput::Bytes(recording.replay_bytes));
        group.bench_with_input(
            BenchmarkId::from_parameter(recording.name),
            &recording.path,
            |b, path| {
                b.iter_custom(|iterations| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        elapsed += runtime.block_on(async {
                            let connection = IbtConnection::builder()
                                .with_path(path.clone())
                                .build()
                                .await
                                .expect("fixture connection should open");
                            let mut frames = Box::pin(
                                connection
                                    .subscribe::<DynamicFrame>()
                                    .expect("fixture subscription should validate"),
                            );

                            let started = Instant::now();
                            connection.start().expect("fixture replay should start");
                            while let Some(frame) = frames.next().await {
                                black_box(frame);
                            }
                            started.elapsed()
                        });
                    }
                    elapsed
                });
            },
        );
    }

    group.finish();
}

fn bench_random_frame_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("ibt_reader_random_frame_read");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.throughput(Throughput::Elements(RANDOM_SEEKS_PER_ITERATION as u64));

    for recording in recordings() {
        recording.prewarm();
        let positions: Vec<_> = (0..RANDOM_SEEKS_PER_ITERATION)
            .map(|index| {
                index.wrapping_mul(1_103_515_245).wrapping_add(12_345) % recording.frame_count
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(recording.name),
            &(recording.path, positions),
            |b, (path, positions)| {
                b.iter_batched(
                    || IbtReader::open(path).expect("fixture should open"),
                    |reader| {
                        for &position in positions {
                            black_box(
                                reader
                                    .frame(black_box(position))
                                    .expect("fixture frame should read"),
                            );
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_open,
    bench_sequential_replay,
    bench_provider_sequential_replay,
    bench_connection_sequential_replay,
    bench_random_frame_read
);
criterion_main!(benches);
