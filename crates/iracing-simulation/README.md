# iracing-simulation

Minimal Rust helpers for checking whether the **iRacing Simulation** is running on a given machine.

This crate talks to iRacing’s local HTTP status endpoint (by default `127.0.0.1:32034`) and also exposes Windows-only process detection helpers:

- A high-level `Simulation` facade with a single “is it running?” check.
- A `SimStatusClient` trait so you can plug in your existing HTTP stack (`reqwest`, `ureq`, etc.).
- A built-in `StdSimStatusClient` that uses raw `TcpStream` and a tiny HTTP parser (no external HTTP dependency required).
- Windows process enumeration helpers for checking whether `iRacingSim64DX11.exe` is running.

## What “running” means

`Simulation::check_sim_status()` mirrors the historical JS semantics used by this project:

- Returns `false` on any client/transport/protocol error.
- Returns `false` on any non-2xx HTTP status code.
- Returns `true` **only** when the response body contains the marker `running:1`.

## Quick start

### Library usage (default client)

```rust
use iracing_simulation::Simulation;

let sim = Simulation::local();
let running = sim.check_sim_status();
println!("running={running}");
```

### Windows process detection

```rust,no_run
#[cfg(windows)]
{
    use iracing_simulation::is_iracing_process_running;

    let running = is_iracing_process_running()?;
    println!("process_running={running}");
}
# Ok::<(), iracing_simulation::ProcessDetectionError>(())
```

### Library usage (custom client)

If you already have an HTTP client in your app, implement `SimStatusClient` and inject it:

```rust
use std::time::Duration;
use iracing_simulation::{Simulation, SimStatusClient, SimStatusResponse, sim_status_url};

struct MyClient;

impl SimStatusClient for MyClient {
    fn get_sim_status(
        &self,
        host: &str,
        port: u16,
        _timeout: Duration,
    ) -> Result<SimStatusResponse, ()> {
        // Use `sim_status_url(host, port)` so the path stays consistent.
        let _url = sim_status_url(host, port);
        Err(())
    }
}

let sim = Simulation::new_with_client("127.0.0.1", 32034, MyClient);
let running = sim.check_sim_status();
```

## Public API overview

Everything below is re-exported from `src/lib.rs`:

### Constants

- `DEFAULT_HOST`: default iRacing status host (`"127.0.0.1"`).
- `DEFAULT_PORT`: default iRacing status port (`32034`).
- `SIM_STATUS_PATH`: endpoint path and query string (`"/get_sim_status?object=simStatus"`).

### URL helper

- `sim_status_url(host, port) -> String`: formats `http://{host}:{port}{SIM_STATUS_PATH}`.

### Response model

- `SimStatusResponse { status_code: u16, body: String }`: minimal shape returned by `SimStatusClient`.

### Client trait (dependency injection seam)

- `trait SimStatusClient`:
  - `get_sim_status(&self, host, port, timeout) -> Result<SimStatusResponse, ()>`

If you need richer error information, wrap it in your own client and map failures to `Err(())` at the boundary.

### High-level facade

- `Simulation` (generic over a `SimStatusClient`, defaults to `StdSimStatusClient`)
  - `Simulation::local()`: uses `DEFAULT_HOST`/`DEFAULT_PORT`.
  - `Simulation::new(host, port)`: default client to arbitrary `host:port`.
  - `Simulation::new_with_client(host, port, client)`: inject your own client.
  - `Simulation::with_timeout(duration)`: override the default timeout (default is 5s).
  - `Simulation::check_sim_status() -> bool`: returns whether iRacing reports `running:1`.

### Windows process helpers

- `DEFAULT_IRACING_PROCESS_NAME`: default Windows executable name for iRacing.
- `is_iracing_process_running() -> Result<bool, ProcessDetectionError>`: convenience wrapper for the default iRacing executable.

Notes:

- Process detection is Windows-only and gated with `#[cfg(windows)]`.
- `Simulation::check_sim_status()` remains an HTTP check; it does not fall back to process detection.
- Basic process enumeration does not normally require administrator privileges.

## Examples (recommended entry point)

The `examples/` directory is the best way to learn usage and expected behavior.

From this crate directory:

```bash
cargo run --example check-connection -- --help
```

From the workspace root:

```bash
cargo run -p iracing-simulation --example check-connection -- --help
```

Included examples:

- `check-connection`: polls until the sim is running (or times out).
- `poll-lifecycle`: logs connect/disconnect transitions.
- `custom-client`: implements `SimStatusClient` using `ureq`.
- `reqwest-blocking`: implements `SimStatusClient` using `reqwest::blocking`.
- `reqwest-async`: implements `SimStatusClient` using `reqwest::Client` + `tokio` bridging.

## How to navigate the crate

- `src/lib.rs`: crate-level docs + public re-exports.
- `src/simulation.rs`: the implementation (endpoint constants, `Simulation`, `StdSimStatusClient`, parser, and unit tests).
- `examples/*.rs`: CLI-oriented sample apps; start here for “how do I use it?”.

## Development

Common commands (run from the workspace root or this crate directory):

```bash
cargo test -p iracing-simulation --all-targets
```

Notes:

- Unit tests use a fake `SimStatusClient` to avoid real sockets.
- The built-in HTTP parser is intentionally small; it extracts the status code from the first line and treats everything after the first `\r\n\r\n` as the body.
