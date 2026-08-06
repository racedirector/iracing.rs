//! Aggregate decoding benchmarks for a captured live telemetry frame.
//!
//! # What these benchmarks measure
//!
//! Each case starts with an already constructed [`iracing_sdk::FramePacket`]
//! and decoded variable schema. The timed work reads values from that packet
//! and materializes newly owned Rust outputs, as a telemetry consumer would do
//! once frame data is available.
//!
//! This complements `var_data_extraction.rs`, which measures individual value
//! decodes. Aggregate measurements include traversal, result construction,
//! output allocation, and output destruction across a realistic workload.
//!
//! `fresh_outputs` means that every iteration creates new decoded result
//! containers. It does not mean that the raw frame or schema is reconstructed.
//! In particular, dynamic arrays allocate nested `Vec<TelemetryValue>` values,
//! while the representative consumer allocates three typed array vectors.
//!
//! # Workloads
//!
//! - `telemetry_value_all/fresh_outputs` dynamically decodes every variable in
//!   the captured schema through [`TelemetryValue::decode`]. This represents a
//!   generic consumer such as an exporter or telemetry inspector.
//! - `representative_consumer/fresh_outputs_47_fields_3_arrays` builds the
//!   shared 47-field typed adapter and separately decodes three 72-car arrays.
//!   This represents an application that selects known fields at compile time.
//! - `telemetry_value_scalars/fresh_outputs` dynamically decodes every scalar
//!   variable but excludes arrays. Comparing it with the all-variable case
//!   shows the combined incremental cost of array element decoding, nested
//!   value construction, allocation, and destruction; it does not isolate
//!   allocation alone.
//!
//! # Timing boundaries and correctness
//!
//! Schema loading, frame generation, deterministic offset/name ordering,
//! adapter validation, variable lookup, and sentinel verification all happen
//! before Criterion starts timing. Timed loops contain only public decoding or
//! adaptation calls and materialization of fresh outputs. Complete outputs are
//! passed to [`std::hint::black_box`] so the compiler cannot discard the work.
//!
//! Setup verifies every captured variable and array element against the
//! deterministic fixture. Missing variables, changed representative array
//! shapes, type mismatches, unsupported types, and decoding errors fail loudly
//! rather than silently reducing benchmark coverage. Frame sizes, offsets, and
//! aggregate schema counts are otherwise read from the checked-in capture and
//! may change between iRacing builds.
//!
//! # Reading results
//!
//! The all-variable case reports bytes per second because it decodes the whole
//! captured schema. Selected and scalar-only workloads report decoded elements
//! per second; claiming the size of the entire backing frame for a selected
//! workload would overstate how much it processes.
//!
//! Results describe decoding from a prepared, in-memory frame on the machine
//! running Criterion. They do not include connection setup, shared-memory or
//! IBT frame acquisition, schema discovery, session parsing, task scheduling,
//! subscription delivery, serialization, storage, networking, rendering, or
//! application-specific computation. They are not end-to-end pipeline latency
//! measurements or proof that the decoder is optimally implemented.
//!
//! Run this target with:
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench aggregate_frame_parsing
//! ```

mod support;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use iracing_sdk::{TelemetryValue, VariableInfo, adapters::FrameAdapter, types::VarData};
use std::hint::black_box;
use support::workloads::{CONSUMER_ELEMENT_COUNT, ConsumerFrame47, prepare_consumer_workload};

/// Decode a prepared metadata list without performing schema name lookups.
fn decode_all(data: &[u8], variables: &[&VariableInfo]) -> Vec<TelemetryValue> {
    variables
        .iter()
        .map(|info| {
            TelemetryValue::decode(data, info).unwrap_or_else(|error| {
                panic!(
                    "aggregate decode failed for `{}` at offset {} with type {:?} and count {}: {error}",
                    info.name, info.offset, info.data_type, info.count
                )
            })
        })
        .collect()
}

/// Measure generic, runtime-typed decoding of every captured schema variable.
fn bench_telemetry_value_all(c: &mut Criterion) {
    let fixture = support::full_frame_fixture();
    let packet = fixture.packet();
    let variables = support::ordered_variables(packet.schema.as_ref());
    support::verify_full_frame(&packet, &variables);
    let total_elements = support::total_elements(&variables);
    assert!(total_elements >= variables.len());

    let mut group = c.benchmark_group("aggregate_full_frame/telemetry_value_all");
    group.throughput(Throughput::Bytes(packet.data.len() as u64));
    group.bench_function("fresh_outputs", |b| {
        b.iter(|| {
            let values = decode_all(black_box(packet.data.as_ref()), black_box(&variables));
            black_box(values)
        })
    });
    group.finish();
}

/// Measure a selected, typed consumer made up of 47 scalars and three arrays.
fn bench_representative_consumer(c: &mut Criterion) {
    let fixture = support::full_frame_fixture();
    let packet = fixture.packet();
    let workload = prepare_consumer_workload(&packet, fixture.schema.as_ref());

    let mut group = c.benchmark_group("aggregate_full_frame/representative_consumer");
    group.throughput(Throughput::Elements(CONSUMER_ELEMENT_COUNT));
    group.bench_function("fresh_outputs_47_fields_3_arrays", |b| {
        b.iter(|| {
            let frame = ConsumerFrame47::adapt(black_box(&packet), black_box(&workload.validation));
            let lap_dist_pct = Vec::<f32>::from_bytes(
                black_box(packet.data.as_ref()),
                black_box(&workload.lap_dist_pct),
            )
            .unwrap_or_else(|error| panic!("aggregate CarIdxLapDistPct decode failed: {error}"));
            let track_surface = Vec::<i32>::from_bytes(
                black_box(packet.data.as_ref()),
                black_box(&workload.track_surface),
            )
            .unwrap_or_else(|error| panic!("aggregate CarIdxTrackSurface decode failed: {error}"));
            let on_pit_road = Vec::<bool>::from_bytes(
                black_box(packet.data.as_ref()),
                black_box(&workload.on_pit_road),
            )
            .unwrap_or_else(|error| panic!("aggregate CarIdxOnPitRoad decode failed: {error}"));

            black_box((frame, lap_dist_pct, track_surface, on_pit_road))
        })
    });
    group.finish();
}

/// Measure runtime-typed scalar decoding without nested array materialization.
fn bench_telemetry_value_scalars(c: &mut Criterion) {
    let fixture = support::full_frame_fixture();
    let packet = fixture.packet();
    let variables = support::ordered_variables(packet.schema.as_ref());
    support::verify_full_frame(&packet, &variables);
    let scalar_variables: Vec<_> = variables
        .into_iter()
        .filter(|info| info.count == 1)
        .collect();
    assert!(!scalar_variables.is_empty());

    let mut group = c.benchmark_group("aggregate_full_frame/telemetry_value_scalars");
    group.throughput(Throughput::Elements(scalar_variables.len() as u64));
    group.bench_function("fresh_outputs", |b| {
        b.iter(|| {
            let values = decode_all(
                black_box(packet.data.as_ref()),
                black_box(&scalar_variables),
            );
            black_box(values)
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_telemetry_value_all,
    bench_representative_consumer,
    bench_telemetry_value_scalars
);
criterion_main!(benches);
