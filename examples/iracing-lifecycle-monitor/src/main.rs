use anyhow::Result;
#[cfg(windows)]
use clap::Parser;
#[cfg(windows)]
use iracing_sdk::WindowsConnection;
#[cfg(windows)]
use iracing_simulation::{Simulation, is_iracing_process_running};
#[cfg(windows)]
use std::{thread, time::Duration};

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
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    #[cfg(not(windows))]
    {
        Err(anyhow::anyhow!(
            "iracing-lifecycle-monitor is only supported on Windows"
        ))
    }

    #[cfg(windows)]
    {
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
}

/// Waits for the iRacing process to start running.
#[cfg(windows)]
fn wait_for_process(poll_interval: Duration) -> Result<()> {
    tracing::info!("Waiting for iRacing process");

    loop {
        // If the iRacing process is running, return
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

/// Waits for the simulation to return that it's running via iracing_simulation.
#[cfg(windows)]
fn wait_for_sim_status(simulation: &Simulation, poll_interval: Duration) -> Result<()> {
    tracing::info!("Waiting for iRacing sim status endpoint to report running");

    // Ensure the process is still running and check the sim status
    loop {
        // If the process is still running, check sim status.
        // If the process is not still running, bail
        // If there is an error checking the process status, sleep and try again.
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

        // Check if the sim is connected
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
        // If the process is not still running (detected process, and sim reports connected), bail
        if !process_still_ready(simulation) {
            tracing::info!("Process or sim status became unavailable before telemetry connected");
            return;
        }

        // Attempt to connect
        match WindowsConnection::try_connect() {
            Ok(connection) => {
                // If not connected, try again
                if !connection.is_connected() {
                    tracing::debug!("Shared memory opened but telemetry is not connected yet");
                    thread::sleep(poll_interval);
                    continue;
                }

                tracing::info!("Telemetry connected");
                monitor_connected_session(simulation, connection, poll_interval, telemetry_wait);
                return;
            }
            Err(_) => {
                continue;
            }
        }
    }
}

#[cfg(windows)]
fn monitor_connected_session(
    simulation: &Simulation,
    connection: WindowsConnection,
    poll_interval: Duration,
    telemetry_wait: Duration,
) {
    tracing::info!("Monitoring live telemetry session");

    loop {
        // Check if the process is running
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

        // Check if the sim is connected
        if !simulation.check_sim_status() {
            tracing::info!("Sim status dropped; restarting monitor");
            return;
        }

        // Check if the telemetry is connected
        if !connection.is_connected() {
            tracing::info!("Telemetry disconnected; restarting monitor");
            return;
        }

        // Wait for an update
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
