//! Checks to see if the iRacing simulation is running on the local host.
//! Will print `running=true` if the simulation is running, else `running=false`.
//!

use std::{
    thread,
    time::{Duration, Instant},
};

use clap::Parser;
use iracing_simulation::{DEFAULT_HOST, DEFAULT_PORT, Simulation};
use tracing_subscriber::EnvFilter;

/// Checks whether the iRacing simulation is running on the local host.
///
/// Polls until either:
/// - The simulation reports running=true
/// - The timeout is reached
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Host address of the iRacing simulation HTTP endpoint
    #[arg(long, default_value = DEFAULT_HOST)]
    host: String,

    /// Port of the iRacing simulation HTTP endpoint
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,

    /// Timeout in seconds.
    /// Use 0 to wait indefinitely.
    #[arg(short, long, default_value_t = 30)]
    timeout: u64,

    /// Poll interval in milliseconds
    #[arg(long, default_value_t = 1000)]
    interval_ms: u64,
}

fn main() {
    // ------------------------------------------------------------
    // Logging initialization.
    // Default to TRACE unless RUST_LOG is set.
    // ------------------------------------------------------------
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // ------------------------------------------------------------
    // Parse CLI arguments
    // ------------------------------------------------------------
    let Args {
        host,
        port,
        timeout,
        interval_ms,
    } = Args::parse();

    let sim = Simulation::new(&host, port);

    let poll_interval = Duration::from_millis(interval_ms);

    let start = Instant::now();

    loop {
        let running = sim.check_sim_status();
        tracing::debug!("running={}", running);

        if running {
            tracing::info!("Simulation is running.");
            return;
        }

        if timeout != 0 && start.elapsed() >= Duration::from_secs(timeout) {
            tracing::debug!("Timed out after {} seconds.", timeout);
            std::process::exit(1);
        }

        thread::sleep(poll_interval);
    }
}
