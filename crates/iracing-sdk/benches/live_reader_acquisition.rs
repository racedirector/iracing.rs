//! Deterministic microbenchmarks for portable live acquisition.
//!
//! The frame length is the 8,586-byte size recorded in the checked-in live
//! variable-schema capture. The source is ordinary mutable memory; these cases
//! do not invoke Win32, map shared memory, wait for events, or model simulator
//! pacing.
//!
//! Timed boundaries:
//!
//! - `direct_copy_floor` validates and allocates one owned frame-region copy.
//! - `unchanged_tick` reads and validates the header but does not allocate or
//!   copy a frame buffer.
//! - `stable_new_tick` performs complete checked acquisition and one accepted
//!   frame copy.
//! - `one_retry` rejects a deliberately torn first copy, rereads the header,
//!   and accepts the second frame copy.
//!
//! Case construction, baseline establishment, source mutation, and result
//! assertions occur outside the timed routines. Byte throughput denotes bytes
//! in an accepted frame; the unchanged case reports observations instead.
//! Allocations are part of direct-copy, stable, and retry acquisition because
//! accepted snapshots own their bytes. Host-specific times are comparison data,
//! not CI thresholds or end-to-end live latency.

use std::{cell::RefCell, hint::black_box};

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use iracing_sdk::{
    IRacingSDKError, Result,
    irsdk::{Header, IRSDK_VERSION, StatusField, VariableBuffer, WireType},
    reader::{
        access_source::{ByteRegion, RandomAccessSource},
        live::LiveReader,
    },
};

const FRAME_LENGTH: usize = 8_586;
const FRAME_0_OFFSET: usize = Header::WIRE_SIZE;
const FRAME_1_OFFSET: usize = FRAME_0_OFFSET + FRAME_LENGTH;
const FRAME_2_OFFSET: usize = FRAME_1_OFFSET + FRAME_LENGTH;
const SOURCE_LENGTH: usize = FRAME_2_OFFSET + FRAME_LENGTH;

struct BenchmarkSource {
    bytes: RefCell<Vec<u8>>,
    replacement_after_frame_copy: RefCell<Option<Vec<u8>>>,
}

impl BenchmarkSource {
    fn stable(tick: i32) -> Self {
        Self {
            bytes: RefCell::new(source_bytes(tick)),
            replacement_after_frame_copy: RefCell::new(None),
        }
    }

    fn with_one_retry(first_tick: i32, second_tick: i32) -> Self {
        Self {
            bytes: RefCell::new(source_bytes(first_tick)),
            replacement_after_frame_copy: RefCell::new(Some(source_bytes(second_tick))),
        }
    }
}

impl RandomAccessSource for BenchmarkSource {
    fn len(&self) -> usize {
        self.bytes.borrow().len()
    }

    fn read_exact_at(&self, offset: usize, destination: &mut [u8]) -> Result<()> {
        let region = ByteRegion::new(offset, destination.len())?;
        self.validate_region(region)?;
        destination.copy_from_slice(&self.bytes.borrow()[offset..region.end()]);

        if offset == FRAME_1_OFFSET && destination.len() == FRAME_LENGTH {
            if let Some(replacement) = self.replacement_after_frame_copy.borrow_mut().take() {
                *self.bytes.borrow_mut() = replacement;
            }
        }
        Ok(())
    }
}

fn live_header(tick: i32) -> Header {
    Header::new(
        IRSDK_VERSION,
        StatusField::CONNECTED,
        60,
        tick,
        0,
        0,
        0,
        0,
        3,
        FRAME_LENGTH as i32,
        tick,
        1,
        [
            VariableBuffer::new(tick.saturating_sub(1), FRAME_0_OFFSET as i32, tick.saturating_sub(1)),
            VariableBuffer::new(tick, FRAME_1_OFFSET as i32, tick),
            VariableBuffer::new(tick.saturating_sub(2), FRAME_2_OFFSET as i32, tick.saturating_sub(2)),
            VariableBuffer::new(0, 0, 0),
        ],
    )
}

fn source_bytes(tick: i32) -> Vec<u8> {
    let mut bytes = vec![0_u8; SOURCE_LENGTH];
    let header = live_header(tick);
    bytes[..Header::WIRE_SIZE].copy_from_slice(wire_bytes(&header));
    bytes[FRAME_1_OFFSET..FRAME_1_OFFSET + FRAME_LENGTH].fill(tick as u8);
    bytes
}

fn wire_bytes<T: WireType>(value: &T) -> &[u8] {
    // SAFETY: `WireType` guarantees a fully initialized representation of
    // exactly `WIRE_SIZE` bytes.
    unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), T::WIRE_SIZE) }
}

fn prepared_reader(source: &BenchmarkSource) -> LiveReader {
    let mut reader = LiveReader::new();
    assert!(reader.next_frame(source).unwrap().is_none());
    reader
}

fn bench_live_reader_acquisition(c: &mut Criterion) {
    let direct_source = BenchmarkSource::stable(1);
    let mut direct = c.benchmark_group("live_reader_direct_copy_floor");
    direct.throughput(Throughput::Bytes(FRAME_LENGTH as u64));
    direct.bench_function("8586_bytes", |b| {
        b.iter(|| {
            let snapshot = direct_source
                .snapshot(black_box(
                    ByteRegion::new(FRAME_1_OFFSET, FRAME_LENGTH).unwrap(),
                ))
                .unwrap();
            black_box(snapshot)
        })
    });
    direct.finish();

    let unchanged_source = BenchmarkSource::stable(1);
    let mut unchanged_reader = prepared_reader(&unchanged_source);
    let mut unchanged = c.benchmark_group("live_reader_unchanged_tick");
    unchanged.throughput(Throughput::Elements(1));
    unchanged.bench_function("observation", |b| {
        b.iter(|| {
            let snapshot = unchanged_reader.next_frame(black_box(&unchanged_source)).unwrap();
            assert!(snapshot.is_none());
        })
    });
    unchanged.finish();

    let mut accepted = c.benchmark_group("live_reader_accepted_acquisition");
    accepted.throughput(Throughput::Bytes(FRAME_LENGTH as u64));
    accepted.bench_function("stable_new_tick", |b| {
        b.iter_batched(
            || {
                let baseline_source = BenchmarkSource::stable(1);
                let reader = prepared_reader(&baseline_source);
                (reader, BenchmarkSource::stable(2))
            },
            |(mut reader, source)| {
                let snapshot = reader.next_frame(black_box(&source)).unwrap();
                black_box(snapshot.expect("stable new frame"));
            },
            BatchSize::SmallInput,
        )
    });
    accepted.bench_function("one_retry", |b| {
        b.iter_batched(
            || {
                let baseline_source = BenchmarkSource::stable(1);
                let reader = prepared_reader(&baseline_source);
                (reader, BenchmarkSource::with_one_retry(2, 3))
            },
            |(mut reader, source)| {
                let snapshot = reader.next_frame(black_box(&source)).unwrap();
                let snapshot = snapshot.expect("second attempt frame");
                if snapshot.tick_count() != 3 {
                    panic!("unexpected retry tick");
                }
                black_box(snapshot);
            },
            BatchSize::SmallInput,
        )
    });
    accepted.finish();
}

criterion_group!(benches, bench_live_reader_acquisition);
criterion_main!(benches);
