//! # reqwest-blocking
//!
//! Demonstrates implementing [`SimStatusClient`] with [`reqwest::blocking::Client`].
//!
//! Use this pattern when your application already carries a `reqwest` blocking
//! client and you want iRacing sim-status checks to share the same client
//! configuration (proxy settings, TLS roots, timeouts, etc.).
//!
//! ## Running
//!
//! ```bash
//! cargo run --example reqwest-blocking
//! ```
//!
//! Exits 0 if the simulation is running, 1 otherwise.

use std::time::Duration;

use iracing_simulation::{
    SimStatusClient, SimStatusError, SimStatusResponse, Simulation, sim_status_url,
};
use reqwest::blocking::Client;

// ---------------------------------------------------------------------------
// BYO client implementation
// ---------------------------------------------------------------------------

/// A [`SimStatusClient`] backed by [`reqwest::blocking::Client`].
struct ReqwestBlockingClient {
    client: Client,
}

impl SimStatusClient for ReqwestBlockingClient {
    fn get_sim_status(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<SimStatusResponse, SimStatusError> {
        let response = self
            .client
            .get(sim_status_url(host, port))
            .timeout(timeout)
            .send()
            .map_err(|err| SimStatusError::Client {
                message: err.to_string(),
            })?;

        let status_code = response.status().as_u16();
        let body = response.text().map_err(|err| SimStatusError::Client {
            message: err.to_string(),
        })?;

        Ok(SimStatusResponse { status_code, body })
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let client = Client::new();
    let sim = Simulation::new_with_client("127.0.0.1", 32034, ReqwestBlockingClient { client });

    let running = sim.check_sim_status();
    println!("running={running}");

    if !running {
        std::process::exit(1);
    }
}
