# iracing.rs

Rust workspace for working with iRacing telemetry and simulation state:

- Read `.ibt` recordings (cross-platform)
- Read live iRacing shared memory (Windows-only, where supported)
- Stream frames through an adapter layer for typed projections
- Generate JSON Schema snapshots (serialized as YAML)
- Probe the sim lifecycle via iRacing’s local HTTP status endpoint
- Reuse shared fixture + test-data helpers across crates

## Crates

- [`crates/iracing-sdk`](crates/iracing-sdk) — low-level telemetry: `.ibt` reader (`IbtReader`), session YAML parsing/caching (`SessionInfoParser`), telemetry decoding (`VarData`/`VariableSchema`), plus Windows-only shared-memory + broadcast tools.
- [`crates/iracing-sdk-adapter`](crates/iracing-sdk-adapter) — streaming layer: `Provider` + `FramePacket` and the two-phase `FrameAdapter` contract (`validate_schema` then `adapt`) for fast per-frame extraction.
- [`crates/iracing-sdk-codegen`](crates/iracing-sdk-codegen) — schema generator binaries (`session-schema`, `disk-variable-schema`, `disk-session-schema`, `car-setup-schema`, …).
- [`crates/iracing-simulation`](crates/iracing-simulation) — dependency-light probe for iRacing’s `get_sim_status` endpoint (`Simulation`, `SimStatusClient`, `StdSimStatusClient`).
- [`crates/test-utils`](crates/test-utils) — fixture discovery + guardrails (Git LFS guidance, `require_*` helpers) used by integration tests.

## Generated schema artifacts

This repo checks in a few schema snapshots at the workspace root:

- [`session-schema.yml`](session-schema.yml) — baseline schema for `iracing_sdk::SessionInfo`
- [`iracing-primitives-schema.yml`](iracing-primitives-schema.yml) — `$defs` bank for `irsdk_*` primitive wrappers (enums/bitflags)
- [`live-session-schema.yml`](live-session-schema.yml) — schema generated from live session YAML (Windows-only)
- [`live-variable-schema.yml`](live-variable-schema.yml) — schema generated from live telemetry variables (Windows-only)

Regenerate them (from the workspace root):

```bash
cargo session-schema -- --output-path ./session-schema.yml
cargo iracing-primitives-schema -- --output-path ./iracing-primitives-schema.yml

# Windows-only
cargo live-session-schema -- --output-path ./live-session-schema.yml
cargo live-variable-schema -- --output-path ./live-variable-schema.yml
```

## Getting Started

```bash
# Clone and enter the workspace
git clone https://github.com/racedirector/iracing.rs
cd iracing.rs

# Install Git LFS once per machine (needed for recorded telemetry fixtures)
git lfs pull

# Build everything
cargo build --workspace

# Run the full test suite
cargo test --workspace --all-targets

# Format and lint gates
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Common Cargo aliases

Defined in `.cargo/config.toml` for convenience:

| Alias | Command | Purpose |
| --- | --- | --- |
| `cargo ibt-to-csv` | `cargo run -p iracing-sdk --bin ibt-to-csv --` | Convert `.ibt` telemetry to CSV. |
| `cargo ibt-session-parser` | `cargo run -p iracing-sdk --bin ibt-session-parser --` | Extract session YAML from `.ibt`. |
| `cargo broadcast-cli` | `cargo run -p iracing-sdk --bin broadcast-cli --` | Send iRacing broadcast commands (Windows). |
| `cargo session-schema` | `cargo run -p iracing-sdk-codegen --bin session-schema --` | Emit baseline session schema. |
| `cargo disk-variable-schema` | `cargo run -p iracing-sdk-codegen --bin disk-variable-schema --` | Generate telemetry schema from `.ibt` headers. |
| `cargo live-session-schema` | `cargo run -p iracing-sdk-codegen --bin live-session-schema --` | Collect live session schema (Windows). |

## Development Notes

- **Platform gates**: Live shared-memory support, broadcast commands, and some codegen binaries are Windows-only. Keep new APIs behind `#[cfg(windows)]` and align `package.metadata.dist.bin.*.targets` with the code.
- **Telemetry decoding**: Always use `VarData::from_bytes` and related helpers; frame data is little-endian and manual decoding tends to drift from the authoritative implementation.
- **Session parsing**: `SessionInfoParser` caches YAML, so reuse it rather than reparsing on every frame.
- **Adapters**: `FrameAdapter::validate_schema` returns an `AdapterValidation` that should pre-resolve every field offset; `adapt` must avoid schema map lookups for per-frame performance.
- **Schema discovery**: When new fields appear, run the appropriate codegen bin with `--discover` and incorporate the results back into `iracing-sdk` to improve typings.
- **Fixtures**: Some integration tests expect `.ibt` fixtures under `test-data/ibt/` (see `crates/test-utils`). If you add new recordings under `test-data/`, place them in `test-data/ibt/` so the shared helpers can find them.

## Testing

- `cargo test -p iracing-sdk --doc` and `RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps` duplicate the `Docs` CI job (doctests, docs, `cargo check` for examples/bins when run manually).
- Use crate-specific invocations like `cargo test -p iracing-sdk -- types::tests::bitfield_constructor_works` to target individual tests.
- Benchmarks (`criterion`) require enabling the `benchmark` feature on the relevant crate, e.g. `cargo bench -p iracing-sdk --features benchmark`.
- Integration tests that rely on telemetry recordings will fail fast with actionable messaging if Git LFS fixtures are missing—run `git lfs pull`.

## Release Workflow

- Tag releases (`v*`) trigger the cargo-dist pipeline defined in `.github/workflows/release.yml`. It builds platform-specific artifacts and can host them back to GitHub Releases.
- Keep dist metadata (`[package.metadata.dist]` sections) aligned with any new binaries or feature gates, especially for Windows-only executables.

## Additional Resources

- Per-crate guidance lives alongside each package (`crates/*/AGENTS.md`). Start there for deep-dive development tips.
- Schema tool usage and examples are documented in `crates/iracing-sdk-codegen/README.md`.
- Telemetry consumer examples reside under `examples/` in the respective crates; run them with `cargo run -p <crate> --example <name> -- --help` to inspect options.
- Release notes and packaging pointers live in `docs/releasing.md`.
