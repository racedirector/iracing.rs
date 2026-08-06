//! Subscriber-scaling benchmark for prospective WS and gRPC telemetry services.
//!
//! # Architectures compared
//!
//! Both cases use the production latest-value telemetry pipeline and produce
//! identical fresh, owned, heterogeneous client projections:
//!
//! - `shared_stream`: one SDK `DynamicFrame` subscription is consumed by a
//!   service-side loop that creates every client projection from that frame.
//! - `stream_per_client`: every client owns an SDK `DynamicFrame` subscription;
//!   the service consumes all streams and projects each resulting frame.
//!
//! Each client requests 12 scalar fields. Plans rotate through the checked-in
//! captured schema, and variable lookup is resolved before timing. The timed
//! path includes provider frame construction, latest-value delivery, stream
//! coordination, scalar decoding, and fresh `Vec<TelemetryValue>` outputs.
//!
//! WS/gRPC serialization, protocol framing, network I/O, per-client queues,
//! slow-client backpressure, and multi-threaded scheduling are intentionally
//! excluded. Those costs should be layered onto this benchmark once a concrete
//! wire representation and service runtime exist.
//!
//! Run with:
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench subscriber_fanout
//! ```

mod support;

use std::{hint::black_box, sync::Arc, time::Instant};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use iracing_sdk::{
    DynamicFrame, TelemetryValue, TelemetryValueProvider, VariableInfo, VariableSchema,
};
use support::telemetry_pipeline::LatestCase;

const REQUESTED_FIELDS: usize = 12;
const CRITERION_CLIENTS: [usize; 6] = [1, 16, 64, 128, 256, 512];
const DIAGNOSTIC_CLIENTS: [usize; 12] = [
    1, 4, 16, 64, 128, 256, 512, 1_024, 2_048, 4_096, 8_192, 16_384,
];
const SOURCE_HZ: f64 = 60.0;

type ProjectionPlan = Vec<VariableInfo>;
type Projection = Vec<TelemetryValue>;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime should build")
}

/// Create heterogeneous, prevalidated client field selections.
fn projection_plans(schema: &VariableSchema, clients: usize) -> Vec<ProjectionPlan> {
    let scalar_variables: Vec<_> = support::ordered_variables(schema)
        .into_iter()
        .filter(|info| info.count == 1)
        .cloned()
        .collect();
    assert!(scalar_variables.len() >= REQUESTED_FIELDS);

    (0..clients)
        .map(|client| {
            let start = client.wrapping_mul(7) % scalar_variables.len();
            (0..REQUESTED_FIELDS)
                .map(|field| scalar_variables[(start + field) % scalar_variables.len()].clone())
                .collect()
        })
        .collect()
}

fn project(frame: &DynamicFrame, plan: &[VariableInfo]) -> Projection {
    plan.iter()
        .map(|info| {
            frame
                .telemetry_value(info)
                .unwrap_or_else(|error| panic!("failed to project `{}`: {error}", info.name))
        })
        .collect()
}

fn shared_stream_duration(
    runtime: &tokio::runtime::Runtime,
    data: Arc<Vec<u8>>,
    schema: Arc<VariableSchema>,
    plans: &[ProjectionPlan],
    frames: usize,
) -> std::time::Duration {
    let mut case = {
        let _runtime_guard = runtime.enter();
        LatestCase::<DynamicFrame>::new(data, schema, 1, None)
    };

    let started = Instant::now();
    runtime.block_on(case.consume_paced(frames, |frame| {
        for plan in plans {
            black_box(project(frame, plan));
        }
    }));
    let elapsed = started.elapsed();

    assert_eq!(
        case.source.reads.load(std::sync::atomic::Ordering::Relaxed),
        frames
    );
    runtime.block_on(case.shutdown());
    elapsed
}

fn stream_per_client_duration(
    runtime: &tokio::runtime::Runtime,
    data: Arc<Vec<u8>>,
    schema: Arc<VariableSchema>,
    plans: &[ProjectionPlan],
    frames: usize,
) -> std::time::Duration {
    let clients = plans.len();
    let mut case = {
        let _runtime_guard = runtime.enter();
        LatestCase::<DynamicFrame>::new(data, schema, clients, None)
    };
    let mut next_client = 0_usize;

    let started = Instant::now();
    runtime.block_on(case.consume_paced(frames, |frame| {
        black_box(project(frame, &plans[next_client]));
        next_client += 1;
        if next_client == clients {
            next_client = 0;
        }
    }));
    let elapsed = started.elapsed();

    assert_eq!(next_client, 0);
    assert_eq!(
        case.source.reads.load(std::sync::atomic::Ordering::Relaxed),
        frames
    );
    runtime.block_on(case.shutdown());
    elapsed
}

fn bench_fanout(c: &mut Criterion) {
    let fixture = support::full_frame_fixture();
    let data = Arc::new(fixture.data);
    let schema = fixture.schema;
    let runtime = runtime();

    let mut shared = c.benchmark_group("subscriber_fanout/shared_stream/12_fields");
    for clients in CRITERION_CLIENTS {
        let plans = projection_plans(&schema, clients);
        shared.throughput(Throughput::Elements(clients as u64));
        shared.bench_function(format!("clients_{clients}"), |b| {
            b.iter_custom(|iterations| {
                shared_stream_duration(
                    &runtime,
                    Arc::clone(&data),
                    Arc::clone(&schema),
                    &plans,
                    usize::try_from(iterations).expect("Criterion iteration overflow"),
                )
            });
        });
    }
    shared.finish();

    let mut independent = c.benchmark_group("subscriber_fanout/stream_per_client/12_fields");
    for clients in CRITERION_CLIENTS {
        let plans = projection_plans(&schema, clients);
        independent.throughput(Throughput::Elements(clients as u64));
        independent.bench_function(format!("clients_{clients}"), |b| {
            b.iter_custom(|iterations| {
                stream_per_client_duration(
                    &runtime,
                    Arc::clone(&data),
                    Arc::clone(&schema),
                    &plans,
                    usize::try_from(iterations).expect("Criterion iteration overflow"),
                )
            });
        });
    }
    independent.finish();
}

fn diagnostic_frames(clients: usize) -> usize {
    (131_072 / clients).clamp(16, 512)
}

fn report_diagnostic(
    architecture: &str,
    clients: usize,
    frames: usize,
    elapsed: std::time::Duration,
) {
    let nanos_per_frame = elapsed.as_nanos() as f64 / frames as f64;
    let source_fps = 1_000_000_000.0 / nanos_per_frame;
    let budget_utilization = nanos_per_frame * SOURCE_HZ / 1_000_000_000.0;
    println!(
        "subscriber_fanout diagnostic architecture={architecture} clients={clients} requested_fields={REQUESTED_FIELDS} frames={frames} ns_per_source_frame={nanos_per_frame:.1} source_fps={source_fps:.1} budget_60hz_pct={:.2} sustains_60hz={}",
        budget_utilization * 100.0,
        budget_utilization <= 1.0,
    );
}

/// Exponential sweep used to locate the approximate 60 Hz saturation point.
fn scaling_diagnostics(_c: &mut Criterion) {
    let fixture = support::full_frame_fixture();
    let data = Arc::new(fixture.data);
    let schema = fixture.schema;
    let runtime = runtime();

    for clients in DIAGNOSTIC_CLIENTS {
        let plans = projection_plans(&schema, clients);
        let frames = diagnostic_frames(clients);
        let elapsed = shared_stream_duration(
            &runtime,
            Arc::clone(&data),
            Arc::clone(&schema),
            &plans,
            frames,
        );
        report_diagnostic("shared_stream", clients, frames, elapsed);

        let elapsed = stream_per_client_duration(
            &runtime,
            Arc::clone(&data),
            Arc::clone(&schema),
            &plans,
            frames,
        );
        report_diagnostic("stream_per_client", clients, frames, elapsed);
    }
}

criterion_group!(benches, scaling_diagnostics, bench_fanout);
criterion_main!(benches);
