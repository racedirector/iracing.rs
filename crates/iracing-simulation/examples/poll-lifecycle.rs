//! # poll-lifecycle
//!
//! Polls the iRacing sim-status endpoint and logs state transitions:
//!
//! 1. Waits until the simulation connects.
//! 2. Waits until the simulation disconnects.
//! 3. Exits.
//!
//! ## Running
//!
//! ```bash
//! cargo run --example poll-lifecycle
//! ```

use std::thread;
use std::time::Duration;

use iracing_simulation::Simulation;
use tracing::info;
use tracing_subscriber::EnvFilter;

fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let sim = Simulation::local();
    let poll_interval = Duration::from_secs(1);

    // --- Phase 1: wait for connection ---
    info!("Waiting for iRacing simulation connection...");
    while !sim.check_sim_status() {
        thread::sleep(poll_interval);
    }

    // --- Phase 2: wait for disconnection ---
    info!("Connection established, waiting for disconnect...");
    while sim.check_sim_status() {
        thread::sleep(poll_interval);
    }

    info!("Disconnected!");
}
