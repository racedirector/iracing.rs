use anyhow::Result;
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use iracing_sdk::WindowsConnection;
#[cfg(windows)]
use iracing_simulation::{DEFAULT_IRACING_PROCESS_NAME, Simulation, is_process_running};
#[cfg(windows)]
use std::{thread, time::Duration};
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
#[derive(Debug, Parser)]
#[command(
    version,
    about = "Monitor iRacing process, sim status, and live telemetry lifecycle"
)]
struct Args {
    /// Executable name to watch for on Windows.
    #[arg(long, default_value = DEFAULT_IRACING_PROCESS_NAME)]
    process_name: String,

    /// Poll interval, in milliseconds, for process and sim-status checks.
    #[arg(long, default_value_t = 1000)]
    poll_interval_ms: u64,

    /// Wait timeout, in milliseconds, for telemetry update checks.
    #[arg(long, default_value_t = 500)]
    telemetry_wait_ms: u64,
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    run()
}

#[cfg(windows)]
fn run() -> Result<()> {
    let args = Args::parse();
    let poll_interval = Duration::from_millis(args.poll_interval_ms);
    let telemetry_wait = Duration::from_millis(args.telemetry_wait_ms);
    let simulation = Simulation::local();

    loop {
        wait_for_process(&args.process_name, poll_interval)?;
        wait_for_sim_status(&simulation, &args.process_name, poll_interval)?;
        monitor_telemetry(
            &simulation,
            &args.process_name,
            poll_interval,
            telemetry_wait,
        );
    }
}

#[cfg(windows)]
fn wait_for_process(process_name: &str, poll_interval: Duration) -> Result<()> {
    tracing::info!(process_name, "Waiting for iRacing process");

    loop {
        if is_process_running(process_name)? {
            tracing::info!(process_name, "Detected iRacing process");
            return Ok(());
        }

        thread::sleep(poll_interval);
    }
}

#[cfg(windows)]
fn wait_for_sim_status(
    simulation: &Simulation,
    process_name: &str,
    poll_interval: Duration,
) -> Result<()> {
    tracing::info!("Waiting for iRacing sim status endpoint to report running");

    loop {
        if !is_process_running(process_name)? {
            tracing::info!("iRacing process exited before sim status became ready");
            return Ok(());
        }

        if simulation.check_sim_status() {
            tracing::info!("Sim status endpoint reports running");
            return Ok(());
        }

        thread::sleep(poll_interval);
    }
}

#[cfg(windows)]
fn monitor_telemetry(
    simulation: &Simulation,
    process_name: &str,
    poll_interval: Duration,
    telemetry_wait: Duration,
) {
    tracing::info!("Waiting for live telemetry shared memory");

    loop {
        if !process_still_ready(simulation, process_name) {
            tracing::info!("Process or sim status became unavailable before telemetry connected");
            return;
        }

        match WindowsConnection::try_connect() {
            Ok(connection) => {
                if !connection.is_connected() {
                    tracing::debug!("Shared memory opened but telemetry is not connected yet");
                    thread::sleep(poll_interval);
                    continue;
                }

                tracing::info!("Telemetry connected");
                monitor_connected_session(simulation, process_name, connection, telemetry_wait);
                return;
            }
            Err(err) => {
                tracing::debug!(error = %err, "Telemetry shared memory not available yet");
                thread::sleep(poll_interval);
            }
        }
    }
}

#[cfg(windows)]
fn monitor_connected_session(
    simulation: &Simulation,
    process_name: &str,
    connection: WindowsConnection,
    telemetry_wait: Duration,
) {
    tracing::info!("Monitoring live telemetry session");

    loop {
        if !is_process_running(process_name).unwrap_or(false) {
            tracing::info!("iRacing process exited; restarting monitor");
            return;
        }

        if !simulation.check_sim_status() {
            tracing::info!("Sim status dropped; restarting monitor");
            return;
        }

        if !connection.is_connected() {
            tracing::info!("Telemetry disconnected; restarting monitor");
            return;
        }

        match connection.wait_for_update(telemetry_wait) {
            Ok(iracing_sdk::WaitResult::Signaled) => {
                tracing::debug!("Telemetry update signaled");
            }
            Ok(iracing_sdk::WaitResult::Timeout) => {
                tracing::debug!("Telemetry wait timed out");
            }
            Err(err) => {
                tracing::warn!(error = %err, "Telemetry wait failed; restarting monitor");
                return;
            }
        }
    }
}

#[cfg(windows)]
fn process_still_ready(simulation: &Simulation, process_name: &str) -> bool {
    match is_process_running(process_name) {
        Ok(true) => simulation.check_sim_status(),
        Ok(false) => false,
        Err(err) => {
            tracing::warn!(error = %err, "Process detection failed");
            false
        }
    }
}

#[cfg(not(windows))]
fn run() -> Result<()> {
    Err(anyhow::anyhow!(
        "iracing-lifecycle-monitor is only supported on Windows"
    ))
}
