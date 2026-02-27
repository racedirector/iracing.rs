//! # custom-client
//!
//! Demonstrates how to plug a third-party HTTP client into [`Simulation`] by
//! implementing the [`SimStatusClient`] trait.
//!
//! The built-in [`StdSimStatusClient`] uses a raw `TcpStream` and has no
//! external dependencies. If you already have `reqwest`, `ureq`, or another
//! HTTP library in your project you can reuse it here — for example to share
//! connection pools, apply retry policies, or add telemetry.
//!
//! ## Running
//!
//! ```bash
//! cargo run --example custom-client
//! ```
//!
//! The example exits with code 0 when the simulation is running and 1 when it
//! is not (or unreachable).

use std::time::Duration;

use iracing_simulation::{Simulation, SimStatusClient, SimStatusResponse, sim_status_url};

// ---------------------------------------------------------------------------
// BYO client implementation
// ---------------------------------------------------------------------------

/// A [`SimStatusClient`] backed by [`ureq`].
///
/// In a real application you would typically wrap your existing HTTP client
/// (reqwest, hyper, ureq, …) so you can share connection pools, apply unified
/// retry / timeout policies, or pipe iRacing health checks through the same
/// observability stack as the rest of your service.
struct UreqClient;

impl SimStatusClient for UreqClient {
    fn get_sim_status(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<SimStatusResponse, ()> {
        let response = ureq::get(&sim_status_url(host, port))
            .timeout(timeout)
            .call()
            .map_err(|_| ())?;

        let status_code = response.status();
        let body = response.into_string().map_err(|_| ())?;

        Ok(SimStatusResponse { status_code, body })
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let sim = Simulation::new_with_client("127.0.0.1", 32034, UreqClient);

    let running = sim.check_sim_status();
    println!("running={running}");

    if !running {
        std::process::exit(1);
    }
}
