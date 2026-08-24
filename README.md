# iracing.rs

_Big thanks to Kevin O'Neill ([werace.au](werace.au)] and his [`pitwall`](https://crates.io/crates/pitwall) and [`pitwall-derive`](https://crates.io/crates/pitwall-derive) library.
This library is heavily influenced by their initial implementation._

Rust workspace for working with iRacing telemetry and simulation state:

- Read `.ibt` recordings (cross-platform)
- Read live iRacing shared memory (Windows-only, where supported)
- Stream frames through adapter APIs for typed projections
- Generate JSON Schema snapshots (serialized as YAML)
- Probe the sim lifecycle via iRacing’s local HTTP status endpoint
- Reuse shared fixture + test-data helpers across crates

## Crates

- [`crates/iracing-sdk`](crates/iracing-sdk) — low-level telemetry plus the streaming adapter APIs: `.ibt` reader (`IbtReader`), session YAML parsing/caching (`SessionInfoParser`), telemetry decoding (`VarData`/`VariableSchema`), `Provider`, `FramePacket`, `FrameAdapter`, `DynamicFrame`, `IbtProvider`, the Windows-only `LiveProvider`, and Windows-only shared-memory + broadcast tools.
- [`crates/iracing-sdk`](crates/iracing-sdk) — also contains the schema generator binaries (`session-schema`, `disk-variable-schema`, `disk-session-schema`, `car-setup-schema`, `live-session-schema`, `live-variable-schema`, …).
- [`crates/iracing-simulation`](crates/iracing-simulation) — dependency-light probe for iRacing’s `get_sim_status` endpoint (`Simulation`, `SimStatusClient`, `StdSimStatusClient`).

## Generated schema artifacts

Schema snapshots are checked in under [`docs/reference`](docs/reference). Do not hand-edit them.
They are also the starting point when constructing test/benchmark schemas,
session YAML, or simulated telemetry frames: consult the
[`docs/reference` usage guide](docs/reference/README.md) before creating fields
or values from memory. The disk/live artifacts record concrete observed
layouts, so keep each snapshot's frame size and offsets together; do not assume
that one capture is an exhaustive schema for every car and session.

| Artifact | Purpose | Regenerate from workspace root |
| --- | --- | --- |
| [`docs/reference/session-schema.yml`](docs/reference/session-schema.yml) | Baseline schema for `iracing_sdk::SessionInfo`. | `cargo session-schema -- --output-path ./docs/reference/session-schema.yml` |
| [`docs/reference/variable-schema.yml`](docs/reference/variable-schema.yml) | Baseline schema for `iracing_sdk::VariableInfo`. | `cargo variable-schema -- --output-path ./docs/reference/variable-schema.yml` |
| [`docs/reference/primitives-schema.yml`](docs/reference/primitives-schema.yml) | `$defs` bank for `irsdk_*` primitive wrappers (enums/bitflags). | `cargo primitives-schema -- --output-path ./docs/reference/primitives-schema.yml` |
| [`docs/reference/disk-variable-schema.yml`](docs/reference/disk-variable-schema.yml) | Telemetry variable schema derived from `.ibt` headers. | `cargo disk-variable-schema -- --ibt-path <PATH_TO_FILE.ibt> --output-path ./docs/reference/disk-variable-schema.yml` |
| [`docs/reference/live-session-schema.yml`](docs/reference/live-session-schema.yml) | Schema generated from live session YAML. Windows-only. | `cargo live-session-schema -- --output-path ./docs/reference/live-session-schema.yml` |
| [`docs/reference/live-variable-schema.yml`](docs/reference/live-variable-schema.yml) | Schema generated from live telemetry variables. Windows-only. | `cargo live-variable-schema -- --output-path ./docs/reference/live-variable-schema.yml` |

## Getting Started

```bash
# Clone and enter the workspace
git clone https://github.com/racedirector/iracing.rs
cd iracing.rs

# Regenerate and verify deterministic telemetry fixtures when needed
python3 scripts/check_test_fixtures.py

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
| `cargo headers` | `cargo run -p iracing-sdk --bin headers --` | Inspect IBT headers, live headers on Windows, or SDK header layouts. |
| `cargo session-schema` | `cargo run -p iracing-sdk --features codegen,schema-discovery --bin session-schema --` | Emit baseline session schema. |
| `cargo variable-schema` | `cargo run -p iracing-sdk --features codegen,schema-discovery --bin variable-schema --` | Emit baseline variable schema. |
| `cargo primitives-schema` | `cargo run -p iracing-sdk --features codegen,schema-discovery --bin primitives-schema --` | Emit the `irsdk_*` primitive schema catalog. |
| `cargo disk-variable-schema` | `cargo run -p iracing-sdk --features codegen,schema-discovery --bin disk-variable-schema --` | Generate telemetry schema from `.ibt` headers. |
| `cargo live-session-schema` | `cargo run -p iracing-sdk --features codegen,schema-discovery --bin live-session-schema --` | Collect live session schema (Windows). |
| `cargo live-variable-schema` | `cargo run -p iracing-sdk --features codegen,schema-discovery --bin live-variable-schema --` | Collect live telemetry variable schema (Windows). |

## Development Notes

- **Platform gates**: Live shared-memory support, broadcast commands, and some codegen binaries are Windows-only. Keep new APIs behind `#[cfg(windows)]` and align `package.metadata.dist.bin.*.targets` with the code.
- **Telemetry decoding**: Always use `VarData::from_bytes` and related helpers; frame data is little-endian and manual decoding tends to drift from the authoritative implementation.
- **Session parsing**: `SessionInfoParser` caches YAML, so reuse it rather than reparsing on every frame.
- **Adapters**: `FrameAdapter::validate_schema` returns an `AdapterValidation` that should pre-resolve every field offset; `adapt` must avoid schema map lookups for per-frame performance. The primary adapter surface is in `crates/iracing-sdk`.
- **Schema discovery**: When new fields appear, run the appropriate codegen bin with `--discover` and incorporate the results back into `iracing-sdk` to improve typings.
- **Fixtures**: Integration tests use deterministic generated `.ibt` fixtures listed in `test-data/ibt/manifest.json` (see `iracing_sdk::test_utils`). Run `python3 scripts/check_test_fixtures.py` after changing fixture profiles.

## Testing

- `cargo test -p iracing-sdk --doc` and `RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps` duplicate the `Docs` CI job (doctests, docs, `cargo check` for examples/bins when run manually).
- Use crate-specific invocations like `cargo test -p iracing-sdk -- types::tests::bitfield_constructor_works` to target individual tests.
- Benchmarks (`criterion`) require enabling the `benchmark` feature on the relevant crate, e.g. `cargo bench -p iracing-sdk --features benchmark`.
- Integration tests that rely on telemetry fixtures will fail fast with actionable messaging if generated fixtures are missing. Regenerate with `python3 scripts/check_test_fixtures.py`.

## Release Workflow

- Tag releases (`v*`) trigger the cargo-dist pipeline defined in `.github/workflows/release.yml`. It builds platform-specific artifacts and can host them back to GitHub Releases.
- Keep dist metadata (`[package.metadata.dist]` sections) aligned with any new binaries or feature gates, especially for Windows-only executables.

## Additional Resources

- Per-crate guidance lives alongside each package (`crates/*/AGENTS.md`). Start there for deep-dive development tips.
- Schema tool usage and examples are documented in the binary sources under `crates/iracing-sdk/src/bin/`.
- Telemetry consumer examples reside under `examples/` in the respective crates; run them with `cargo run -p <crate> --example <name> -- --help` to inspect options.
- Release notes and packaging pointers live in `docs/releasing.md`.
