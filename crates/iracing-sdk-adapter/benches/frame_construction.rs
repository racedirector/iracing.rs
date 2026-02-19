//! Benchmarks for core frame packet construction
//!
//! Tests the <100μs frame construction latency goal for:
//! - FramePacket creation with real telemetry data from IBT files
//! - Arc<[u8]> cloning overhead for zero-copy data sharing
//! - Tick count operations and wraparound handling
//!
//! Platform: Cross-platform (uses real IBT test files, CI-safe)

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use iracing_sdk_adapter::{FramePacket, IbtReader, VariableSchema};
use std::hint::black_box;
use std::sync::Arc;
use test_utils::get_smallest_ibt_test_file;

/// Load real telemetry data from IBT test file
fn load_real_frame_data() -> (Vec<u8>, u32, u32, Arc<VariableSchema>) {
    let ibt_file = get_smallest_ibt_test_file().expect("No IBT test files found");
    let mut reader = IbtReader::open(&ibt_file).expect("Failed to open IBT file");

    let schema = Arc::new(reader.variables().clone());

    let (data, tick, session_version) = reader
        .read_next_frame()
        .expect("Failed to read frame")
        .expect("No frames in IBT");

    (data, tick, session_version, schema)
}

fn bench_frame_packet_creation(c: &mut Criterion) {
    let (data, tick, session_version, schema) = load_real_frame_data();

    let mut group = c.benchmark_group("frame_packet_creation");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("new_from_ibt_data", |b| {
        b.iter(|| {
            let packet = FramePacket::new(
                black_box(data.clone()),
                black_box(tick),
                black_box(session_version),
                black_box(Arc::clone(&schema)),
            );
            black_box(packet)
        })
    });

    group.finish();
}

fn bench_arc_cloning(c: &mut Criterion) {
    let (data, tick, session_version, schema) = load_real_frame_data();
    let packet = FramePacket::new(data, tick, session_version, Arc::clone(&schema));

    c.bench_function("arc_clone_frame_data", |b| {
        b.iter(|| {
            let data_ref = black_box(&packet.data);
            let cloned = black_box(Arc::clone(data_ref));
            black_box(cloned)
        })
    });

    c.bench_function("arc_clone_schema", |b| {
        b.iter(|| {
            let schema_ref = black_box(&packet.schema);
            let cloned = black_box(Arc::clone(schema_ref));
            black_box(cloned)
        })
    });
}

fn bench_tick_operations(c: &mut Criterion) {
    let (data, tick, session_version, schema) = load_real_frame_data();

    let mut group = c.benchmark_group("tick_operations");

    let packet = FramePacket::new(data, tick, session_version, Arc::clone(&schema));

    group.bench_function("tick_access", |b| {
        b.iter(|| {
            let tick = black_box(packet.tick);
            black_box(tick)
        })
    });

    // Test tick wraparound comparison
    group.bench_function("tick_comparison", |b| {
        b.iter(|| {
            let tick1 = black_box(u32::MAX - 100);
            let tick2 = black_box(100u32);
            let is_newer = black_box(tick2.wrapping_sub(tick1) < u32::MAX / 2);
            black_box(is_newer)
        })
    });

    group.finish();
}

fn bench_frame_construction_latency(c: &mut Criterion) {
    let (data, tick, session_version, schema) = load_real_frame_data();

    let mut group = c.benchmark_group("frame_construction_latency");

    // This is the critical benchmark for the <100μs goal
    // Tests end-to-end: Vec<u8> allocation + FramePacket construction
    group.bench_function("full_frame_pipeline", |b| {
        b.iter(|| {
            let data_copy = black_box(data.clone());
            let packet = FramePacket::new(
                black_box(data_copy),
                black_box(tick),
                black_box(session_version),
                black_box(Arc::clone(&schema)),
            );
            black_box(packet)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_frame_packet_creation,
    bench_arc_cloning,
    bench_tick_operations,
    bench_frame_construction_latency
);
criterion_main!(benches);
