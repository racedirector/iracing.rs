//! Performance baseline for the public [`IbtReader`] storage API.
//!
//! This target is intentionally compatible with both the complete-file reader
//! and the file-backed reader. Keep its case names and timed boundaries stable
//! so Criterion can compare results across the storage refactor.
//!
//! Run only this focused target with:
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench ibt_reader_performance
//! ```
//!
//! # Timed boundaries
//!
//! - `ibt_reader_open` includes opening the path, reading whatever the reader
//!   implementation requires, parsing metadata, and dropping the reader.
//! - `ibt_reader_sequential_replay` constructs the reader outside the timed
//!   routine, then reads every owned frame through `read_next_frame()`.
//! - `ibt_provider_sequential_replay` constructs the replay provider outside
//!   the timed routine, then requests frames through `Provider::next_frame()`.
//! - `ibt_connection_sequential_replay` constructs the public disk connection
//!   and one dynamic-frame subscription outside the timed routine, then starts
//!   and drains the coordinated replay stream.
//! - `ibt_reader_random_seek` constructs the reader outside the timed routine,
//!   then performs 1,024 deterministic `seek_to_frame()` calls. It does not
//!   read frames after seeking.
//!
//! Results include normal operating-system filesystem behavior and may be
//! affected by a warm page cache. Compare revisions on the same machine with
//! the same fixtures and build profile. These timing benchmarks do not measure
//! retained heap; the #84 measurement layer records that separately.

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use futures::StreamExt;
use iracing_sdk::{
    DynamicFrame, IbtConnection, ibt::IbtReader, provider::Provider, providers::ibt::IbtProvider,
};
use std::{hint::black_box, path::PathBuf, time::Duration, time::Instant};

const RANDOM_SEEKS_PER_ITERATION: usize = 1_024;

struct Recording {
    name: &'static str,
    path: PathBuf,
    file_size: u64,
    replay_bytes: u64,
    frame_count: usize,
}

impl Recording {
    fn load(name: &'static str, relative_path: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative_path);
        let file_size = std::fs::metadata(&path)
            .unwrap_or_else(|error| panic!("could not stat {}: {error}", path.display()))
            .len();
        let reader = IbtReader::open(&path)
            .unwrap_or_else(|error| panic!("could not open {}: {error}", path.display()));
        let frame_count = reader.total_frames();
        let frame_size = u64::try_from(reader.header().buffer_length)
            .expect("validated IBT frame size should fit u64");
        let replay_bytes = frame_size
            .checked_mul(u64::try_from(frame_count).expect("frame count should fit u64"))
            .expect("fixture replay byte count should fit u64");

        Self {
            name,
            path,
            file_size,
            replay_bytes,
            frame_count,
        }
    }
}

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
        group.throughput(Throughput::Bytes(recording.replay_bytes));
        group.bench_with_input(
            BenchmarkId::from_parameter(recording.name),
            &recording.path,
            |b, path| {
                b.iter_batched(
                    || IbtReader::open(path).expect("fixture should open"),
                    |mut reader| {
                        while let Some(frame) =
                            reader.read_next_frame().expect("fixture frame should read")
                        {
                            black_box(frame);
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

fn bench_random_seek(c: &mut Criterion) {
    let mut group = c.benchmark_group("ibt_reader_random_seek");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.throughput(Throughput::Elements(RANDOM_SEEKS_PER_ITERATION as u64));

    for recording in recordings() {
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
                    |mut reader| {
                        for &position in positions {
                            reader
                                .seek_to_frame(black_box(position))
                                .expect("fixture seek should succeed");
                        }
                        black_box(reader.current_frame())
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
    bench_random_seek
);
criterion_main!(benches);
