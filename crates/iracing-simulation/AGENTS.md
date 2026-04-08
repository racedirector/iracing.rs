# AGENTS.md

Cheat sheet for OpenCode when editing `crates/iracing-simulation`.

## Critical Commands
- `cargo test -p iracing-simulation --all-targets` runs the library + doctests.
- Example smoke tests: `cargo run -p iracing-simulation --example poll-lifecycle -- --host 127.0.0.1` (see `examples/` for full arg lists).

## Library Surface
- `Simulation::local()` connects to `DEFAULT_HOST`/`DEFAULT_PORT` with the dependency-free `StdSimStatusClient`; use it for end-to-end examples.
- Inject custom HTTP stacks by implementing `SimStatusClient` and passing it through `Simulation::new_with_client`; unit tests rely on this seam to avoid real sockets.
- `check_sim_status()` mirrors the historical JS semantics: returns `true` only on 2xx responses containing `running:1`.
- Keep `sim_status_url()` as the single source of truth for endpoint formatting—callers and tests should not hardcode the path.

## Testing Tips
- Tests should prefer fake clients over live network calls; see `simulation::tests` for the minimal `FakeClient` pattern.
- When touching the raw TCP client, add coverage that exercises the HTTP parser (`parse_http_response`) with realistic responses.

## Examples
- `poll-lifecycle`, `reqwest-async`, `reqwest-blocking`, `custom-client`, and `check-connection` demonstrate different HTTP clients; run them via `cargo run -p iracing-simulation --example <name> -- --help` to inspect options.
- Keep examples dependency-light—only the examples should pull in `reqwest`/`ureq`; the library itself stays dependency-free for the default path.
