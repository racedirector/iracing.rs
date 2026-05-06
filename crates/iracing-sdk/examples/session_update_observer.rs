#[cfg(windows)]
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
#[cfg(windows)]
use iracing_sdk::SessionInfo;
#[cfg(windows)]
use iracing_sdk::WaitResult;
#[cfg(windows)]
use iracing_sdk::WindowsConnection;
#[cfg(windows)]
use serde_json::Value;

fn main() -> Result<()> {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    #[cfg(not(windows))]
    {
        tracing::warn!(
            "session-update-observer is only supported on Windows because it depends on iRacing's Windows shared memory APIs."
        );
        Err(anyhow!(
            "session-update-observer is only supported on Windows"
        ))
    }

    #[cfg(windows)]
    {
        tracing::info!("Opening iRacing connection...");

        let connection = WindowsConnection::try_connect().expect("Failed to connect to iRacing");
        if !connection.is_connected() {
            return Err(anyhow!("iRacing telemetry is not connected"));
        }

        let mut previous_session_info_update: i32 = -1;
        let mut previous_session_info: Option<SessionInfo> = None;
        loop {
            if !connection.is_connected() {
                return Err(anyhow!("iRacing disconnected"));
            }

            match connection.wait_for_update(Duration::from_millis(500)) {
                Ok(WaitResult::Signaled) => {
                    let current_update = connection.session_info_update();

                    if current_update != previous_session_info_update
                        && let Some(session_info_yaml) = connection.session_info()
                    {
                        let session_info =
                            SessionInfo::parse(&session_info_yaml).map_err(|err| {
                                anyhow!(
                                    "failed to parse session info update {current_update}: {err}"
                                )
                            })?;

                        log_session_info_diff(previous_session_info.as_ref(), &session_info)?;
                        previous_session_info = Some(session_info);
                        previous_session_info_update = current_update;
                    }
                    continue;
                }
                Ok(WaitResult::Timeout) => continue,
                Err(err) => return Err(anyhow!("{}", err)),
            }
        }
    }
}

#[cfg(windows)]
fn log_session_info_diff(previous: Option<&SessionInfo>, current: &SessionInfo) -> Result<()> {
    match previous {
        Some(previous) => {
            let previous_value = serde_json::to_value(previous)?;
            let current_value = serde_json::to_value(current)?;
            let mut changes = Vec::new();

            collect_value_diff("$", &previous_value, &current_value, &mut changes);

            if changes.is_empty() {
                tracing::info!(
                    "Session info update received, but no parsed field changes were detected."
                );
            } else {
                tracing::info!("Session info update changed {} field(s):", changes.len());
                for change in changes {
                    tracing::info!("{}", change);
                }
            }
        }
        None => tracing::info!("Captured initial session info snapshot."),
    }

    Ok(())
}

#[cfg(windows)]
fn collect_value_diff(path: &str, previous: &Value, current: &Value, changes: &mut Vec<String>) {
    match (previous, current) {
        (Value::Object(previous), Value::Object(current)) => {
            for (key, previous_value) in previous {
                let next_path = format!("{path}.{key}");
                match current.get(key) {
                    Some(current_value) => {
                        collect_value_diff(&next_path, previous_value, current_value, changes);
                    }
                    None => changes.push(format!("{next_path}: removed (was {previous_value})")),
                }
            }

            for (key, current_value) in current {
                if !previous.contains_key(key) {
                    let next_path = format!("{path}.{key}");
                    changes.push(format!("{next_path}: added {current_value}"));
                }
            }
        }
        (Value::Array(previous), Value::Array(current)) => {
            let shared_len = previous.len().min(current.len());

            for index in 0..shared_len {
                let next_path = format!("{path}[{index}]");
                collect_value_diff(&next_path, &previous[index], &current[index], changes);
            }

            for (index, previous_value) in previous.iter().enumerate().skip(shared_len) {
                changes.push(format!("{path}[{index}]: removed (was {previous_value})"));
            }

            for (index, current_value) in current.iter().enumerate().skip(shared_len) {
                changes.push(format!("{path}[{index}]: added {current_value}"));
            }
        }
        _ if previous != current => {
            changes.push(format!("{path}: {previous} -> {current}"));
        }
        _ => {}
    }
}
