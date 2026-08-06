//! Manual benchmarks for the Windows live telemetry path.
//!
//! # Requirements
//!
//! This target requires Windows and an active iRacing session producing live
//! telemetry. It exits on non-Windows platforms and is not suitable for stable
//! unattended CI comparisons. Before running it:
//!
//! 1. Start iRacing and enter a session that is actively producing telemetry.
//! 2. Close avoidable background workloads that could disturb scheduling.
//! 3. Run:
//!
//!    ```text
//!    cargo bench -p iracing-sdk --features benchmark --bench live_frame_latency
//!    ```
//!
//! The target prints available CPU, core-count, RAM, Windows, and simulator
//! process information so results can be interpreted in their environment.
//!
//! # What is being measured
//!
//! These cases exercise real subscriptions and therefore mix library work with
//! simulator availability, source pacing, Tokio scheduling, and operating-system
//! wake-up latency:
//!
//! - `live_frame_construction/shared_memory_to_packet` creates a new
//!   `DynamicFrame` subscription inside each timed iteration and waits for its
//!   first item. It includes subscription setup, validation, waiting, delivery,
//!   and dynamic-view adaptation; it is not isolated packet construction.
//! - `live_sustained_throughput/subscription_setup` measures creation of a live
//!   `DynamicFrame` subscription without waiting for a frame.
//! - `live_sustained_throughput/frame_delivery_rate` creates one subscription
//!   before timing, then awaits one item per iteration. At `UpdateRate::Native`,
//!   simulator frame cadence can dominate the reported time.
//! - `live_sustained_throughput/burst_collection_100ms` creates each subscription
//!   in Criterion's untimed batch setup, then counts delivered items during a
//!   timed 100 ms wall-clock window.
//! - `live_adapter_pipeline/full_live_pipeline` creates a new typed five-field
//!   subscription per iteration and waits for its first adapted item. It includes
//!   subscription setup and source waiting as well as typed adaptation.
//!
//! Connection creation and system-information collection happen before the
//! corresponding timed group. Received values and counts are passed to
//! [`std::hint::black_box`] so the compiler cannot discard the work.
//!
//! # Reading results
//!
//! Do not compare these timings directly with in-memory decoding or packet
//! construction microbenchmarks. They measure different boundaries, and several
//! cases intentionally include waiting for an external 60 Hz source. Results can
//! vary with simulator state, telemetry cadence, Windows scheduling, hardware,
//! other subscribers, and background load.
//!
//! The cases observe delivered frames but do not independently prove that no
//! upstream frames were coalesced or dropped. They also exclude downstream
//! serialization, networking, storage, rendering, and application computation.

#[cfg(windows)]
use criterion::{Criterion, criterion_group, criterion_main};
#[cfg(windows)]
use futures::StreamExt;
#[cfg(windows)]
use iracing_sdk::{DynamicFrame, IRacingTelemetryFrame, LiveConnection, UpdateRate};
#[cfg(windows)]
use std::hint::black_box;
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
/// Print environmental context outside all Criterion timed loops.
fn print_system_info() {
    use std::process::Command;

    println!("\n=== System Information ===");

    // CPU information
    if let Ok(output) = Command::new("wmic").args(["cpu", "get", "name"]).output()
        && let Ok(cpu_info) = String::from_utf8(output.stdout)
    {
        let cpu = cpu_info.lines().nth(1).unwrap_or("Unknown").trim();
        println!("CPU: {}", cpu);
    }

    // Core count
    if let Ok(cores) = std::thread::available_parallelism() {
        println!("CPU Cores: {}", cores);
    }

    // RAM information
    if let Ok(output) = Command::new("wmic")
        .args(["computersystem", "get", "totalphysicalmemory"])
        .output()
        && let Ok(ram_info) = String::from_utf8(output.stdout)
        && let Some(ram_bytes) = ram_info
            .lines()
            .nth(1)
            .and_then(|s| s.trim().parse::<u64>().ok())
    {
        let ram_gb = ram_bytes / (1024 * 1024 * 1024);
        println!("RAM: {} GB", ram_gb);
    }

    // Windows version
    if let Ok(output) = Command::new("cmd").args(["/c", "ver"]).output()
        && let Ok(version) = String::from_utf8(output.stdout)
    {
        println!("Windows: {}", version.trim());
    }

    // Check if iRacing is running
    if let Ok(output) = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq iRacingSim64DX11.exe"])
        .output()
        && let Ok(tasklist) = String::from_utf8(output.stdout)
    {
        if tasklist.contains("iRacingSim64DX11.exe") {
            println!("iRacing: Running");
        } else {
            println!("iRacing: NOT RUNNING (benchmarks will fail)");
        }
    }

    println!("==========================\n");
}

#[cfg(windows)]
fn iracing_is_running() -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq iRacingSim64DX11.exe"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|output| output.contains("iRacingSim64DX11.exe"))
}

#[cfg(windows)]
/// Measure first-item delivery after creating a dynamic subscription per iteration.
fn bench_live_frame_construction(c: &mut Criterion) {
    print_system_info();
    if !iracing_is_running() {
        eprintln!("Skipping manual live benchmark because iRacing is not running");
        return;
    }

    // Attempt to connect to live telemetry
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let connection = {
        let _runtime_guard = runtime.enter();
        LiveConnection::builder().build()
    };

    let connection = match connection {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("\n❌ Failed to connect to iRacing: {}", e);
            eprintln!("   Make sure iRacing is running and you're in a session");
            eprintln!("   These benchmarks require an active iRacing connection\n");
            return;
        }
    };

    println!("✅ Connected to iRacing successfully\n");

    let mut group = c.benchmark_group("live_frame_construction");

    // Benchmark frame extraction from shared memory using subscribe API
    group.bench_function("shared_memory_to_packet", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let mut stream = Box::pin(
                    connection
                        .subscribe::<DynamicFrame>(UpdateRate::Native)
                        .expect("live DynamicFrame subscription should validate"),
                );
                if let Some(frame) = stream.next().await {
                    black_box(frame);
                }
            })
        })
    });

    group.finish();
}

#[cfg(windows)]
/// Separate subscription setup, ongoing delivery, and fixed-window collection.
fn bench_live_sustained_throughput(c: &mut Criterion) {
    if !iracing_is_running() {
        return;
    }
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let connection = {
        let _runtime_guard = runtime.enter();
        LiveConnection::builder().build()
    };

    let connection = match connection {
        Ok(conn) => conn,
        Err(_) => return, // Already reported error in previous benchmark
    };

    let mut group = c.benchmark_group("live_sustained_throughput");

    // Benchmark 1: Subscription setup overhead
    group.bench_function("subscription_setup", |b| {
        b.iter(|| {
            // Measure just the subscription creation cost
            let _stream = connection.subscribe::<DynamicFrame>(UpdateRate::Native);
            let _ = black_box(_stream);
        })
    });

    // Benchmark 2: Pure frame delivery throughput (eliminates setup overhead)
    group.measurement_time(Duration::from_secs(10));
    group.bench_function("frame_delivery_rate", |b| {
        // Create subscription ONCE outside the benchmark loop
        let mut stream = Box::pin(
            connection
                .subscribe::<DynamicFrame>(UpdateRate::Native)
                .expect("live DynamicFrame subscription should validate"),
        );

        b.iter(|| {
            runtime.block_on(async {
                // Just fetch one frame - measures pure delivery latency
                if let Some(frame) = stream.next().await {
                    black_box(frame);
                }
            })
        })
    });

    // Benchmark 3: Burst collection (how many frames in 100ms window)
    group.measurement_time(Duration::from_secs(5));
    group.bench_function("burst_collection_100ms", |b| {
        b.iter_batched(
            || {
                // Setup: create fresh subscription for each sample
                Box::pin(
                    connection
                        .subscribe::<DynamicFrame>(UpdateRate::Native)
                        .expect("live DynamicFrame subscription should validate"),
                )
            },
            |mut stream| {
                // Measurement: collect frames for 100ms
                runtime.block_on(async {
                    let mut frames_received = 0;
                    let deadline = tokio::time::Instant::now() + Duration::from_millis(100);

                    while tokio::time::Instant::now() < deadline {
                        if let Some(frame) = stream.next().await {
                            frames_received += 1;
                            black_box(frame);
                        }
                    }

                    black_box(frames_received)
                })
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

#[cfg(windows)]
/// Measure first-item delivery through a new typed subscription per iteration.
fn bench_live_adapter_pipeline(c: &mut Criterion) {
    if !iracing_is_running() {
        return;
    }
    // Simple test adapter for live data
    #[allow(dead_code)]
    #[derive(IRacingTelemetryFrame, Debug)]
    struct LiveTestFrame {
        #[field_name = "Speed"]
        speed: f32,
        #[field_name = "RPM"]
        rpm: f32,
        #[field_name = "Gear"]
        gear: i32,
        #[field_name = "Throttle"]
        throttle: f32,
        #[field_name = "Brake"]
        brake: f32,
    }

    let runtime = tokio::runtime::Runtime::new().unwrap();

    let connection = {
        let _runtime_guard = runtime.enter();
        LiveConnection::builder().build()
    };

    let connection = match connection {
        Ok(conn) => conn,
        Err(_) => return,
    };

    let mut group = c.benchmark_group("live_adapter_pipeline");

    // Subscription setup, source wait, delivery, and typed adaptation.
    group.bench_function("full_live_pipeline", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let mut stream = Box::pin(
                    connection
                        .subscribe::<LiveTestFrame>(UpdateRate::Native)
                        .expect("live adapter subscription should validate"),
                );
                if let Some(frame) = stream.next().await {
                    black_box(frame);
                }
            })
        })
    });

    group.finish();
}

#[cfg(windows)]
criterion_group!(
    benches,
    bench_live_frame_construction,
    bench_live_sustained_throughput,
    bench_live_adapter_pipeline
);

#[cfg(windows)]
criterion_main!(benches);

// Non-Windows stub
#[cfg(not(windows))]
fn main() {
    eprintln!("❌ Live telemetry benchmarks are Windows-only");
    eprintln!("   Run on Windows with iRacing active for live performance testing");
    std::process::exit(1);
}
