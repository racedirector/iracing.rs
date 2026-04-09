//! # reqwest-async
//!
//! Demonstrates implementing [`SimStatusClient`] with [`reqwest::Client`]
//! (the async variant), bridged into the synchronous trait via
//! [`tokio::task::block_in_place`].
//!
//! ## When to use this pattern
//!
//! If your application is already async (tokio-based) and has a shared
//! `reqwest::Client` for other outbound requests, you can reuse it here so
//! iRacing sim-status checks share connection pools, middleware, and TLS
//! configuration without pulling in the `blocking` feature of `reqwest`.
//!
//! `block_in_place` tells tokio that the current worker thread is about to
//! block. Tokio moves all other tasks off that thread while the call runs,
//! preventing the runtime from stalling. This requires the **multi-thread**
//! scheduler (the default for `#[tokio::main]`).
//!
//! ## Running
//!
//! ```bash
//! cargo run --example reqwest-async
//! ```
//!
//! Exits 0 if the simulation is running, 1 otherwise.

use std::time::Duration;

use iracing_simulation::{SimStatusClient, SimStatusResponse, Simulation, sim_status_url};
use tokio::runtime::Handle;
use tokio::task;

// ---------------------------------------------------------------------------
// BYO client implementation
// ---------------------------------------------------------------------------

/// A [`SimStatusClient`] backed by [`reqwest::Client`] (async).
///
/// The async HTTP call is driven to completion with [`task::block_in_place`]
/// so it fits the synchronous [`SimStatusClient`] contract. Because
/// `block_in_place` requires a multi-threaded tokio runtime, this client
/// must be used from a `#[tokio::main]` context (the default scheduler) or
/// any other multi-thread runtime — not `#[tokio::main(flavor = "current_thread")]`.
struct ReqwestAsyncClient {
    client: reqwest::Client,
}

impl SimStatusClient for ReqwestAsyncClient {
    fn get_sim_status(
        &self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<SimStatusResponse, ()> {
        let url = sim_status_url(host, port);
        let client = self.client.clone();

        // Pause this worker thread and drive the async request to completion.
        // Other tokio tasks continue running on the remaining threads.
        task::block_in_place(|| {
            Handle::current().block_on(async move {
                let response = client
                    .get(&url)
                    .timeout(timeout)
                    .send()
                    .await
                    .map_err(|_| ())?;

                let status_code = response.status().as_u16();
                let body = response.text().await.map_err(|_| ())?;

                Ok(SimStatusResponse { status_code, body })
            })
        })
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let client = reqwest::Client::new();
    let sim = Simulation::new_with_client("127.0.0.1", 32034, ReqwestAsyncClient { client });

    let running = sim.check_sim_status();
    println!("running={running}");

    if !running {
        std::process::exit(1);
    }
}
