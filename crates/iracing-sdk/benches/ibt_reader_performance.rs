//! Performance baseline for the public [`IbtReader`] storage API.
//!
//! This target measures the current file-backed reader. Its open and sequential
//! replay cases retain the historical case names used for the storage refactor.
//! The random-access case now reads each selected owned frame; unlike the old
//! seek-only case, it is not directly comparable with that historical result.
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
//!   routine, then reads every owned frame by increasing index.
//! - `ibt_reader_random_access` constructs the reader outside the timed
//!   routine, then reads 1,024 owned frames at deterministic random indices.
//!
//! Results include normal operating-system filesystem behavior and may be
//! affected by a warm page cache. Compare revisions on the same machine with
//! the same fixtures and build profile. These timing benchmarks do not measure
//! retained heap; the #84 measurement layer records that separately.

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use iracing_sdk::ibt::IbtReader;
use std::{hint::black_box, path::PathBuf, time::Duration};

const RANDOM_READS_PER_ITERATION: usize = 1_024;

struct Recording {
    name: &'static str,
    path: PathBuf,
    replay_bytes: u64,
    frame_count: usize,
    frame_size: u64,
}

impl Recording {
    fn load(name: &'static str, relative_path: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative_path);
        let reader = IbtReader::open(&path)
            .unwrap_or_else(|error| panic!("could not open {}: {error}", path.display()));
        let frame_count = reader.layout().frame_count();
        let frame_size = u64::try_from(reader.layout().frame_size())
            .expect("validated IBT frame size should fit u64");
        let replay_bytes = frame_size
            .checked_mul(u64::try_from(frame_count).expect("frame count should fit u64"))
            .expect("fixture replay byte count should fit u64");

        Self {
            name,
            path,
            replay_bytes,
            frame_count,
            frame_size,
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
                    || {
                        let reader = IbtReader::open(path).expect("fixture should open");
                        let frame_count = reader.layout().frame_count();
                        (reader, frame_count)
                    },
                    |(mut reader, frame_count)| {
                        for frame_index in 0..frame_count {
                            let frame = reader
                                .frame(black_box(frame_index))
                                .expect("fixture frame should read");
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

fn bench_random_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("ibt_reader_random_access");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for recording in recordings() {
        let positions: Vec<_> = (0..RANDOM_READS_PER_ITERATION)
            .map(|index| {
                index.wrapping_mul(1_103_515_245).wrapping_add(12_345) % recording.frame_count
            })
            .collect();
        let bytes_per_iteration = recording
            .frame_size
            .checked_mul(
                u64::try_from(RANDOM_READS_PER_ITERATION)
                    .expect("random-read count should fit u64"),
            )
            .expect("random-read byte count should fit u64");
        group.throughput(Throughput::Bytes(bytes_per_iteration));

        group.bench_with_input(
            BenchmarkId::from_parameter(recording.name),
            &(recording.path, positions),
            |b, (path, positions)| {
                b.iter_batched(
                    || IbtReader::open(path).expect("fixture should open"),
                    |mut reader| {
                        for &position in positions {
                            let frame = reader
                                .frame(black_box(position))
                                .expect("fixture frame should read");
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

criterion_group!(
    benches,
    bench_open,
    bench_sequential_replay,
    bench_random_access
);
criterion_main!(benches);
