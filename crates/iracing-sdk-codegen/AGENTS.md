# AGENTS.md

Focused tips for OpenCode when touching `crates/iracing-sdk-codegen`.

## Critical Commands
- Use the workspace Cargo aliases (`cargo session-schema`, `cargo disk-variable-schema`, etc.) from `.cargo/config.toml`; they map 1:1 to these bins and forward extra args.
- When running directly, prefer `cargo run -p iracing-sdk-codegen --bin <tool> -- --help` to confirm flags.
- All binaries rely on `iracing-sdk` with `codegen` + `schema-discovery`; that dependency already enables both features—do not disable them locally.

## Platform Notes
- `live-variable-schema`, `live-session-schema`, and live mode of `car-setup-schema` must stay `#[cfg(windows)]`. Cargo-dist targets for those bins are already Windows-only; update both the source gate and `package.metadata.dist.bin.*.targets` if you add new live tools.
- Non-Windows runs of live-oriented bins should surface the explicit unsupported-platform error rather than panic.

## Usage Patterns
- Schema generators emit YAML-serialized JSON Schema; diff flows (`--diff`, `--diff-output-path`) expect canonical baseline files from `session-schema`.
- `--discover` aggregates unknown fields into the output—remember to ship those updates back into `iracing-sdk` when they represent new telemetry.
- `--annotate` injects unit metadata (`x-irsdk-unit-ref`, etc.); downstream tooling assumes those annotations exist when requested.

## Testing & Validation
- There are no dedicated unit tests; treat each bin like an integration test by wiring it against representative `.ibt` fixtures from `test-data/ibt/` (run `git lfs pull` first).
- Before publishing schemas, run the relevant bin twice: once with `--diff` against the baseline to ensure only expected fields changed, then without `--diff` to produce the final artifact.
