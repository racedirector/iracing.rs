//! Simulator-backed owned-frame acquisition diagnostic. Waits occur outside timing.
//! Run with `cargo bench -p iracing-sdk --features benchmark --bench live-acquisition-diagnostic -- --profile LABEL`.

#[cfg(windows)]
#[path = "support/live.rs"]
mod live;

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use anyhow::{Context, anyhow, bail};
    use iracing_sdk::{VariableSchema, WaitResult, WindowsConnection};
    use serde_json::json;
    use std::{collections::HashMap, hint::black_box, time::Instant};

    let options = live::Options::parse()?;
    let mut connection = WindowsConnection::try_connect()
        .context("iRacing shared memory is unavailable; start an active session")?;
    if !connection.is_connected() {
        bail!("iRacing has no active session");
    }
    let header = connection.header();
    let tick_hz = header.tick_rate;
    if tick_hz <= 0 {
        bail!("invalid source tick rate");
    }
    let frame_size = usize::try_from(header.buffer_length)?;
    let initial_session_version = header.session_info_update;
    let variables = connection.get_variables()?;
    let schema = VariableSchema::new(
        variables
            .into_iter()
            .map(|v| (v.name.clone(), v))
            .collect::<HashMap<_, _>>(),
        frame_size,
    )?;
    if schema.variables.is_empty() {
        bail!("live session has no telemetry variable schema");
    }
    let initial_fingerprint = live::fingerprint(&schema)?;
    let mut run = live::new_run(&options, &schema, tick_hz)?;

    let target = options.warmup_frames + options.target_frames;
    let deadline = Instant::now() + options.timeout;
    let mut accepted = 0_usize;
    let mut attempted = 0_usize;
    let mut no_frame_polls = 0_usize;
    let mut wait_signals = 0_usize;
    let mut wait_timeouts = 0_usize;
    let mut copied_bytes = 0_u64;
    let mut skipped_ticks = 0_u64;
    let mut previous_tick = None;
    let mut samples = Vec::with_capacity(options.target_frames);
    let mut sampling_started = None;
    let mut failure = None;

    while accepted < target && Instant::now() < deadline {
        if !connection.is_connected() {
            failure = Some("simulator disconnected");
            break;
        }
        match connection.wait_for_update(std::time::Duration::from_millis(100))? {
            WaitResult::Signaled => wait_signals += 1,
            WaitResult::Timeout => wait_timeouts += 1,
        }
        // This is the pre-#111 provider-equivalent boundary, including the owned copy.
        let start = Instant::now();
        let owned = connection.get_new_data().map(<[u8]>::to_vec);
        let result = owned.map(|owned| {
            let header = connection.header();
            let slot = connection.find_latest_buffer(header);
            let tick = header.buffers[slot].tick_count;
            let session_version = header.session_info_update;
            (owned, tick, session_version)
        });
        let elapsed_ns = start.elapsed().as_nanos() as u64;
        attempted += 1;
        let Some((owned, tick, session_version)) = result else {
            no_frame_polls += 1;
            continue;
        };
        if owned.len() != frame_size || connection.header().buffer_length != frame_size as i32 {
            failure = Some("frame geometry changed");
            break;
        }
        if session_version != initial_session_version || connection.header().tick_rate != tick_hz {
            failure = Some("session version or source rate changed");
            break;
        }
        let current = connection.get_variables()?;
        let current_schema = VariableSchema::new(
            current.into_iter().map(|v| (v.name.clone(), v)).collect(),
            frame_size,
        )?;
        if live::fingerprint(&current_schema)? != initial_fingerprint {
            failure = Some("schema changed");
            break;
        }
        black_box((owned, tick, session_version));
        accepted += 1;
        if accepted > options.warmup_frames {
            if sampling_started.is_none() {
                sampling_started = Some(Instant::now());
            }
            samples.push(elapsed_ns);
            copied_bytes += frame_size as u64;
            if let Some(previous) = previous_tick {
                let advance = tick.wrapping_sub(previous);
                skipped_ticks += u64::try_from(advance.saturating_sub(1)).unwrap_or(0);
            }
            previous_tick = Some(tick);
        }
    }

    let complete = failure.is_none() && samples.len() == options.target_frames;
    let elapsed_s = sampling_started.map_or(0.0, |start| start.elapsed().as_secs_f64());
    let (p50, p95, p99) = if samples.is_empty() {
        (None, None, None)
    } else {
        (
            Some(live::percentile(&mut samples.clone(), 0.50)),
            Some(live::percentile(&mut samples.clone(), 0.95)),
            Some(live::percentile(&mut samples, 0.99)),
        )
    };
    let raw_samples_ns = samples.clone();
    run.cases.push(live::Case {
        id: "live_acquisition/owned_frame",
        experiment_version: 1,
        status: if complete { "complete" } else { "incomplete" },
        samples: accepted.saturating_sub(options.warmup_frames),
        parameters: json!({"timed_boundary": "get_new_data+to_vec+provider_metadata"}),
        metrics: json!({
            "p50_us": p50, "p95_us": p95, "p99_us": p99,
            "attempted": attempted, "accepted_total": accepted,
            "accepted_frames": accepted.saturating_sub(options.warmup_frames),
            "no_frame_polls": no_frame_polls, "wait_signals": wait_signals,
            "wait_timeouts": wait_timeouts, "copied_bytes": copied_bytes,
            "samples_ns": raw_samples_ns,
            "elapsed_s": elapsed_s, "effective_hz": if elapsed_s > 0.0 { Some(samples.len() as f64 / elapsed_s) } else { None },
            "skipped_ticks": skipped_ticks, "failure": failure,
        }),
    });
    let path = live::write_run(&run, &options)?;
    println!("live acquisition report: {}", path.display());
    if !complete {
        return Err(anyhow!(
            "capture incomplete: {}",
            failure.unwrap_or("timeout or insufficient frames")
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    println!("live-acquisition-diagnostic requires Windows and an active iRacing session");
}
