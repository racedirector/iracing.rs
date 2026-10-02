//! Manual live subscription cadence diagnostic. Sampling excludes setup.

#[cfg(windows)]
#[path = "support/live.rs"]
mod live;

#[cfg(windows)]
#[derive(Default)]
struct Subscriber {
    received: usize,
    skipped_ticks: u64,
    last_tick: Option<u32>,
    last_arrival: Option<std::time::Instant>,
    intervals_ns: Vec<u64>,
}

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    use anyhow::{Context, anyhow};
    use futures::{StreamExt, stream::select_all};
    use iracing_sdk::{
        DynamicFrame, LiveConnection, SchemaProvider, UpdateRate, VariableSchema, WindowsConnection,
    };
    use serde_json::json;
    use std::time::Instant;

    let options = live::Options::parse()?;
    let connection = LiveConnection::builder()
        .build()
        .context("start iRacing and enter an active session")?;
    let tick_hz = connection.source_hz() as i32;
    let mut run = live::new_run(&options, connection.schema(), tick_hz)?;
    let initial_fingerprint = run.scenario.schema_fingerprint.clone();
    let monitor = WindowsConnection::try_connect()?;
    let initial_session = monitor.session_info_update();

    for subscriber_count in [1_usize, 4] {
        let streams = (0..subscriber_count)
            .map(|index| {
                let stream = connection.subscribe::<DynamicFrame>(UpdateRate::Native)?;
                Ok(Box::pin(stream.map(move |frame| (index, frame))))
            })
            .collect::<iracing_sdk::Result<Vec<_>>>()?;
        let mut streams = select_all(streams);
        let mut subscribers: Vec<Subscriber> = (0..subscriber_count)
            .map(|_| Subscriber::default())
            .collect();
        let mut first_tick = None;
        let mut warmup_tick = None;
        let mut latest_tick = None;
        let mut first_arrival = None;
        let mut last_arrival = None;
        let mut failure = None;
        let deadline = Instant::now() + options.timeout;

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                failure = Some("timeout");
                break;
            }
            let Some((index, frame)) = tokio::time::timeout(remaining, streams.next())
                .await
                .ok()
                .flatten()
            else {
                failure = Some("live telemetry stream ended or timed out");
                break;
            };
            let arrival = Instant::now();
            let tick = frame.tick_count();
            let origin = *first_tick.get_or_insert(tick);
            if tick.wrapping_sub(origin) < options.warmup_frames as u32 {
                continue;
            }
            let warm = *warmup_tick.get_or_insert(tick);
            if first_arrival.is_none() {
                first_arrival = Some(arrival);
            }
            last_arrival = Some(arrival);
            let observation = &mut subscribers[index];
            if let Some(previous) = observation.last_tick {
                let advance = tick.wrapping_sub(previous);
                if advance == 0 {
                    continue;
                }
                observation.skipped_ticks += u64::from(advance.saturating_sub(1));
            }
            if let Some(previous) = observation.last_arrival {
                observation
                    .intervals_ns
                    .push(arrival.duration_since(previous).as_nanos() as u64);
            }
            observation.received += 1;
            observation.last_tick = Some(tick);
            observation.last_arrival = Some(arrival);
            latest_tick = Some(latest_tick.map_or(tick, |latest: u32| latest.max(tick)));
            if tick.wrapping_sub(warm) >= options.target_frames as u32 {
                break;
            }
        }

        let current_header = monitor.header();
        let current_variables = monitor.get_variables()?;
        let current_schema = VariableSchema::new(
            current_variables
                .into_iter()
                .map(|v| (v.name.clone(), v))
                .collect(),
            run.scenario.frame_size,
        )?;
        if !monitor.is_connected()
            || live::fingerprint(&current_schema)? != initial_fingerprint
            || current_header.buffer_length as usize != run.scenario.frame_size
            || current_header.tick_rate != tick_hz
        {
            failure = Some("disconnected or schema, geometry, or source rate changed");
        }
        if monitor.session_info_update() != initial_session {
            failure = Some("session version changed");
        }
        let opportunities = match (warmup_tick, latest_tick) {
            (Some(first), Some(last)) => last.wrapping_sub(first) as usize + 1,
            _ => 0,
        };
        let complete = failure.is_none()
            && opportunities >= options.target_frames
            && subscribers.iter().all(|s| s.received > 1);
        let elapsed_s = match (first_arrival, last_arrival) {
            (Some(first), Some(last)) => last.duration_since(first).as_secs_f64(),
            _ => 0.0,
        };
        let observations: Vec<_> = subscribers
            .iter_mut()
            .enumerate()
            .map(|(index, subscriber)| {
                let intervals = &mut subscriber.intervals_ns;
                let raw_intervals_ns = intervals.clone();
                json!({
                    "index": index, "received": subscriber.received,
                    "skipped_or_coalesced_ticks": subscriber.skipped_ticks,
                    "p50_us": (!intervals.is_empty()).then(|| live::percentile(&mut intervals.clone(), 0.50)),
                    "p95_us": (!intervals.is_empty()).then(|| live::percentile(&mut intervals.clone(), 0.95)),
                    "p99_us": (!intervals.is_empty()).then(|| live::percentile(intervals, 0.99)),
                    "effective_hz": if elapsed_s > 0.0 { Some(subscriber.received as f64 / elapsed_s) } else { None },
                    "intervals_ns": raw_intervals_ns,
                })
            })
            .collect();
        let received: Vec<_> = subscribers.iter().map(|s| s.received).collect();
        let skew = received.iter().max().unwrap_or(&0) - received.iter().min().unwrap_or(&0);
        let first = observations
            .first()
            .ok_or_else(|| anyhow!("no subscribers"))?;
        run.cases.push(live::Case {
            id: if subscriber_count == 1 { "live_consumer/dynamic_1" } else { "live_consumer/dynamic_4" },
            experiment_version: 1,
            status: if complete { "complete" } else { "incomplete" },
            samples: opportunities,
            parameters: json!({"subscribers": subscriber_count, "adapter": "DynamicFrame", "delivery": "latest-wins"}),
            metrics: json!({
                "p50_us": first["p50_us"], "p95_us": first["p95_us"], "p99_us": first["p99_us"],
                "source_tick_opportunities": opportunities, "elapsed_s": elapsed_s,
                "subscriber_count_skew": skew, "subscribers": observations, "failure": failure,
            }),
        });
    }
    let path = live::write_run(&run, &options)?;
    println!("live consumer report: {}", path.display());
    if run.cases.iter().any(|case| case.status != "complete") {
        return Err(anyhow!(
            "one or more live consumer captures were incomplete"
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    println!("live-telemetry-diagnostic requires Windows and an active iRacing session");
}
