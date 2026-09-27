//! Manual live-telemetry cadence diagnostic.
//!
//! This is not a statistical library benchmark: simulator pacing, Windows
//! scheduling, and machine load dominate the observations. It samples one live
//! subscription and reports inter-arrival jitter plus skipped/coalesced ticks.
//!
//! Run on Windows with an active iRacing session:
//!
//! ```text
//! cargo bench -p iracing-sdk --features benchmark --bench live-telemetry-diagnostic
//! ```

#[cfg(windows)]
use anyhow::{Context, Result, anyhow};

#[cfg(windows)]
fn percentile(samples: &mut [u64], quantile: f64) -> u64 {
    samples.sort_unstable();
    let index = ((samples.len() - 1) as f64 * quantile).round() as usize;
    samples[index]
}

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    use futures::StreamExt;
    use iracing_sdk::{DynamicFrame, LiveConnection, UpdateRate};
    use std::time::{Duration, Instant};

    const TARGET_FRAMES: usize = 600;
    const TIMEOUT: Duration = Duration::from_secs(30);

    println!(
        "live telemetry diagnostic cores={} target_frames={TARGET_FRAMES} timeout_s={}",
        std::thread::available_parallelism().map_or(1, usize::from),
        TIMEOUT.as_secs(),
    );

    let connection = LiveConnection::builder()
        .build()
        .context("failed to connect; start iRacing and enter an active session")?;
    let mut stream = Box::pin(connection.subscribe::<DynamicFrame>(UpdateRate::Native)?);
    let started = Instant::now();

    let (mut intervals, skipped_ticks) = tokio::time::timeout(TIMEOUT, async {
        let mut intervals = Vec::with_capacity(TARGET_FRAMES.saturating_sub(1));
        let mut previous_arrival = None;
        let mut previous_tick = None;
        let mut skipped_ticks = 0u64;

        for _ in 0..TARGET_FRAMES {
            let frame = stream
                .next()
                .await
                .ok_or_else(|| anyhow!("live telemetry stream ended before sampling completed"))?;
            let arrived = Instant::now();

            if let Some(previous) = previous_arrival {
                intervals.push(arrived.duration_since(previous).as_nanos() as u64);
            }
            if let Some(previous) = previous_tick {
                let advance = frame.tick_count().wrapping_sub(previous);
                skipped_ticks += u64::from(advance.saturating_sub(1));
            }

            previous_arrival = Some(arrived);
            previous_tick = Some(frame.tick_count());
        }

        Ok::<_, anyhow::Error>((intervals, skipped_ticks))
    })
    .await
    .context("timed out waiting for live telemetry frames")??;

    if intervals.is_empty() {
        return Err(anyhow!(
            "not enough frames were received to calculate cadence"
        ));
    }

    let elapsed = started.elapsed();
    let p50 = percentile(&mut intervals, 0.50);
    let p95 = percentile(&mut intervals, 0.95);
    let p99 = percentile(&mut intervals, 0.99);
    let effective_hz = (TARGET_FRAMES - 1) as f64 / elapsed.as_secs_f64();

    println!(
        "live telemetry cadence frames={TARGET_FRAMES} elapsed_ms={} effective_hz={effective_hz:.2} p50_interval_us={:.1} p95_interval_us={:.1} p99_interval_us={:.1} skipped_or_coalesced_ticks={skipped_ticks}",
        elapsed.as_millis(),
        p50 as f64 / 1_000.0,
        p95 as f64 / 1_000.0,
        p99 as f64 / 1_000.0,
    );

    Ok(())
}

#[cfg(not(windows))]
fn main() {
    println!("live-telemetry-diagnostic requires Windows and an active iRacing session");
}
