use anyhow::Result;
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use iracing_sdk::WindowsConnection;
#[cfg(windows)]
use iracing_simulation::{Simulation, is_iracing_process_running};
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
        wait_for_process(poll_interval)?;
        wait_for_sim_status(&simulation, poll_interval)?;
        monitor_telemetry(&simulation, poll_interval, telemetry_wait);
    }
}

#[cfg(windows)]
fn wait_for_process(poll_interval: Duration) -> Result<()> {
    tracing::info!("Waiting for iRacing process");

    loop {
        match is_iracing_process_running() {
            Ok(true) => {
                tracing::info!("Detected iRacing process");
                return Ok(());
            }
            Ok(false) => {
                thread::sleep(poll_interval);
            }
            Err(err) => {
                tracing::warn!(error = %err, "Process detection failed while waiting for iRacing; retrying");
                thread::sleep(poll_interval);
            }
        }
    }
}

#[cfg(windows)]
fn wait_for_sim_status(simulation: &Simulation, poll_interval: Duration) -> Result<()> {
    tracing::info!("Waiting for iRacing sim status endpoint to report running");

    loop {
        match is_iracing_process_running() {
            Ok(true) => {}
            Ok(false) => {
                tracing::info!("iRacing process exited before sim status became ready");
                return Ok(());
            }
            Err(err) => {
                tracing::warn!(error = %err, "Process detection failed while waiting for sim status; retrying");
                thread::sleep(poll_interval);
                continue;
            }
        }
        }

        if simulation.check_sim_status() {
            tracing::info!("Sim status endpoint reports running");
            return Ok(());
        }

        thread::sleep(poll_interval);
    }
}

#[cfg(windows)]
fn monitor_telemetry(simulation: &Simulation, poll_interval: Duration, telemetry_wait: Duration) {
    tracing::info!("Waiting for live telemetry shared memory");

    loop {
        if !process_still_ready(simulation) {
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
                monitor_connected_session(simulation, connection, poll_interval, telemetry_wait);
                return;
            }

fn monitor_connected_session(
    simulation: &Simulation,
    connection: WindowsConnection,
    poll_interval: Duration,
    telemetry_wait: Duration,
) {
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
    connection: WindowsConnection,
    telemetry_wait: Duration,
) {
    tracing::info!("Monitoring live telemetry session");

    loop {
        match is_iracing_process_running() {
            Ok(true) => {}
            Ok(false) => {
                tracing::info!("iRacing process exited; restarting monitor");
                return;
            }
            Err(err) => {
                tracing::error!(error = %err, "Process detection failed while monitoring telemetry; retrying");
                thread::sleep(poll_interval);
                continue;
            }
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
fn process_still_ready(simulation: &Simulation) -> bool {
    match is_iracing_process_running() {
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
