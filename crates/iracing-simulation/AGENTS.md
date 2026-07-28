# AGENTS.md

Cheat sheet for agents editing `crates/iracing-simulation`.

## Critical Commands

- `cargo test -p iracing-simulation --all-targets` runs unit tests and example/target test harnesses; use `cargo test -p iracing-simulation --doc` for doctests.
- `RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-simulation --no-deps` and `cargo check -p iracing-simulation --examples --bins` mirror this crate's docs CI checks.
- Example smoke tests: `cargo run -p iracing-simulation --example poll-lifecycle -- --host 127.0.0.1` (see `examples/` for full arg lists).

## Library Surface

- `Simulation::local()` connects to `DEFAULT_HOST`/`DEFAULT_PORT` with `StdSimStatusClient`; use it for end-to-end examples.
- Inject custom HTTP stacks by implementing `SimStatusClient` and passing it through `Simulation::new_with_client`; unit tests rely on this seam to avoid real sockets.
- `check_sim_status()` mirrors the historical JS semantics: returns `true` only on 2xx responses containing `running:1` and collapses transport/protocol errors to `false`.
- Keep `sim_status_url()` as the single source of truth for endpoint formatting—callers and tests should not hardcode the path.
- `process.rs` is Windows-only and re-exported behind `#[cfg(windows)]`; keep the portable HTTP probe independent from process detection.

## Testing Tips

- Tests should prefer fake clients over live network calls; see `simulation::tests` for the minimal `FakeClient` pattern.
- When touching the raw TCP client, add coverage that exercises `parse_http_response` with realistic status lines, header termination, and malformed responses.
- Process-list tests should keep pure string/matching logic separate from Win32 enumeration wherever possible.

## Examples

- `poll-lifecycle`, `reqwest-async`, `reqwest-blocking`, `custom-client`, and `check-connection` demonstrate different HTTP clients; run them via `cargo run -p iracing-simulation --example <name> -- --help` to inspect options.
- Keep the built-in status path free of an external HTTP-client dependency—`reqwest` and `ureq` belong in dev-dependencies/examples. The library still intentionally depends on shared CLI, error, and tracing crates.
