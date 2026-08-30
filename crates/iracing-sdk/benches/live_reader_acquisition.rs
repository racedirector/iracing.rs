//! Deterministic microbenchmarks for concrete live acquisition.
//!
//! The 8,586-byte frame length comes from the checked-in live schema. These
//! cases use a real `MappedView` over stable in-process storage; they do not
//! invoke Win32, event waits, simulator pacing, providers, or delivery.

use std::{cell::RefCell, hint::black_box, ptr::NonNull};

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use iracing_sdk::{
    irsdk::{Header, IRSDK_VERSION, StatusField, VariableBuffer, WireType},
    reader::live::{LiveReader, MappedView},
};

const FRAME_LENGTH: usize = 8_586;
const FRAME_0_OFFSET: usize = Header::WIRE_SIZE;
const FRAME_1_OFFSET: usize = FRAME_0_OFFSET + FRAME_LENGTH;
const FRAME_2_OFFSET: usize = FRAME_1_OFFSET + FRAME_LENGTH;
const SOURCE_LENGTH: usize = FRAME_2_OFFSET + FRAME_LENGTH;

struct BenchmarkMapping {
    bytes: RefCell<Box<[u8]>>,
    replacement_after_frame_copy: RefCell<Option<Box<[u8]>>>,
}

impl BenchmarkMapping {
    fn stable(tick: i32) -> Self {
        Self {
            bytes: RefCell::new(source_bytes(tick).into_boxed_slice()),
            replacement_after_frame_copy: RefCell::new(None),
        }
    }

    fn replace_with_tick(&self, tick: i32) {
        self.bytes.borrow_mut().copy_from_slice(&source_bytes(tick));
    }

    fn replace_with_tick_after_frame_copy(&self, tick: i32) {
        *self.replacement_after_frame_copy.borrow_mut() =
            Some(source_bytes(tick).into_boxed_slice());
    }

    fn with_view<T>(&self, operation: impl FnOnce(&MappedView<'_>) -> T) -> T {
        let (base, length) = {
            let bytes = self.bytes.borrow();
            (
                NonNull::new(bytes.as_ptr().cast_mut()).expect("nonempty benchmark mapping"),
                bytes.len(),
            )
        };
        let observer = |offset, length| self.after_copy(offset, length);
        // SAFETY: The boxed allocation remains stable and readable throughout
        // `operation`; same-sized replacements never move it.
        let view =
            unsafe { MappedView::from_raw_parts(base, length) }.with_copy_observer(&observer);
        operation(&view)
    }

    fn after_copy(&self, offset: usize, length: usize) {
        if offset == FRAME_1_OFFSET
            && length == FRAME_LENGTH
            && let Some(replacement) = self.replacement_after_frame_copy.borrow_mut().take()
        {
            self.bytes.borrow_mut().copy_from_slice(&replacement);
        }
    }

    fn direct_frame_copy(&self) -> Vec<u8> {
        self.bytes.borrow()[FRAME_1_OFFSET..FRAME_1_OFFSET + FRAME_LENGTH].to_vec()
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
            VariableBuffer::new(
                tick.saturating_sub(1),
                FRAME_0_OFFSET as i32,
                tick.saturating_sub(1),
            ),
            VariableBuffer::new(tick, FRAME_1_OFFSET as i32, tick),
            VariableBuffer::new(
                tick.saturating_sub(2),
                FRAME_2_OFFSET as i32,
                tick.saturating_sub(2),
            ),
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

fn prepared_reader(mapping: &BenchmarkMapping) -> LiveReader {
    let mut reader = mapping.with_view(LiveReader::new).unwrap();
    assert!(
        mapping
            .with_view(|view| reader.next_frame(view))
            .unwrap()
            .is_none()
    );
    reader
}

fn bench_live_reader_acquisition(c: &mut Criterion) {
    let direct_mapping = BenchmarkMapping::stable(1);
    let mut direct = c.benchmark_group("live_reader_direct_copy_floor");
    direct.throughput(Throughput::Bytes(FRAME_LENGTH as u64));
    direct.bench_function("8586_bytes", |b| {
        b.iter(|| black_box(direct_mapping.direct_frame_copy()))
    });
    direct.finish();

    let unchanged_mapping = BenchmarkMapping::stable(1);
    let mut unchanged_reader = prepared_reader(&unchanged_mapping);
    let mut unchanged = c.benchmark_group("live_reader_unchanged_tick");
    unchanged.throughput(Throughput::Elements(1));
    unchanged.bench_function("observation", |b| {
        b.iter(|| {
            let snapshot = unchanged_mapping
                .with_view(|view| unchanged_reader.next_frame(black_box(view)))
                .unwrap();
            assert!(snapshot.is_none());
        })
    });
    unchanged.finish();

    let mut accepted = c.benchmark_group("live_reader_accepted_acquisition");
    accepted.throughput(Throughput::Bytes(FRAME_LENGTH as u64));
    accepted.bench_function("stable_new_tick", |b| {
        b.iter_batched(
            || {
                let mapping = BenchmarkMapping::stable(1);
                let reader = prepared_reader(&mapping);
                mapping.replace_with_tick(2);
                (reader, mapping)
            },
            |(mut reader, mapping)| {
                let snapshot = mapping
                    .with_view(|view| reader.next_frame(black_box(view)))
                    .unwrap();
                black_box(snapshot.expect("stable new frame"));
            },
            BatchSize::SmallInput,
        )
    });
    accepted.bench_function("one_retry", |b| {
        b.iter_batched(
            || {
                let mapping = BenchmarkMapping::stable(1);
                let reader = prepared_reader(&mapping);
                mapping.replace_with_tick(2);
                mapping.replace_with_tick_after_frame_copy(3);
                (reader, mapping)
            },
            |(mut reader, mapping)| {
                let snapshot = mapping
                    .with_view(|view| reader.next_frame(black_box(view)))
                    .unwrap()
                    .expect("second attempt frame");
                assert_eq!(snapshot.tick_count(), 3);
                black_box(snapshot);
            },
            BatchSize::SmallInput,
        )
    });
    accepted.finish();
}

criterion_group!(benches, bench_live_reader_acquisition);
criterion_main!(benches);
