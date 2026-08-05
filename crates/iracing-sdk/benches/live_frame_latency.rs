//! Benchmarks for Windows live telemetry frame latency
//!
//! **MANUAL TESTING ONLY** - Requires running iRacing
//!
//! Tests the <100μs frame construction latency goal for:
//! - End-to-end latency: shared memory → FramePacket
//! - Live provider frame construction
//! - Sustained 60Hz throughput with zero drops
//!
//! Platform: Windows-only (requires active iRacing session)
//!
//! ## Running These Benchmarks
//!
//! 1. Start iRacing and load into a session (practice, race, etc.)
//! 2. Run: `just bench-live` or `cargo bench --bench live_frame_latency`
//! 3. Results will include machine specifications for comparison
//!
//! ## Machine Spec Reporting
//!
//! Benchmark output includes:
//! - CPU model and core count
//! - System RAM
//! - Windows version
//! - iRacing version (if detectable)

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
fn bench_live_frame_construction(c: &mut Criterion) {
    print_system_info();

    // Attempt to connect to live telemetry
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let connection = LiveConnection::builder().build();

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
fn bench_live_sustained_throughput(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let connection = LiveConnection::builder().build();

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
fn bench_live_adapter_pipeline(c: &mut Criterion) {
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

    let connection = LiveConnection::builder().build();

    let connection = match connection {
        Ok(conn) => conn,
        Err(_) => return,
    };

    let mut group = c.benchmark_group("live_adapter_pipeline");

    // End-to-end: shared memory → FramePacket → Adapter using subscribe API
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
