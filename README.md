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

- [`crates/iracing-irsdk`](crates/iracing-irsdk) — dependency-light Rust representations of the native SDK wire contract: fixed-layout headers, variable metadata, constants, enums, flags, and broadcast command values.
- [`crates/iracing-sdk`](crates/iracing-sdk) — low-level telemetry plus the streaming adapter APIs: `.ibt` reader (`IbtReader`), session YAML parsing (`SessionInfo::parse`), telemetry decoding (`VarData`/`TelemetryLayout`), `Provider`, `FramePacket`, `FrameAdapter`, `DynamicFrame`, `IbtProvider`, the Windows-only `LiveProvider`, and Windows-only shared-memory + broadcast tools.
- [`crates/iracing-sdk-cli`](crates/iracing-sdk-cli) — the consolidated `iracing-sdk` CLI for telemetry export, snapshots, session schemas, metadata, and broadcast commands.
- [`crates/iracing-simulation`](crates/iracing-simulation) — dependency-light probe for iRacing’s `get_sim_status` endpoint (`Simulation`, `SimStatusClient`, `StdSimStatusClient`).
- [`crates/test-fixtures`](crates/test-fixtures) — unpublished Rust tooling for deterministic `.ibt` fixture generation, verification, and drift checks.

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
| [`docs/reference/session-schema.yml`](docs/reference/session-schema.yml) | Baseline schema for `iracing_sdk::schema::SessionInfo`. | `cargo iracing-sdk session schema type --output ./docs/reference/session-schema.yml` |
| [`docs/reference/variable-schema.yml`](docs/reference/variable-schema.yml) | Historical variable metadata reference schema. | Generator deferred during CLI consolidation. |
| [`docs/reference/primitives-schema.yml`](docs/reference/primitives-schema.yml) | `$defs` bank for `irsdk_*` primitive wrappers (enums/bitflags). | Generator deferred during CLI consolidation. |
| [`docs/reference/disk-variable-schema.yml`](docs/reference/disk-variable-schema.yml) | Telemetry variable schema derived from `.ibt` headers. | Generator deferred during CLI consolidation. |
| [`docs/reference/live-session-schema.yml`](docs/reference/live-session-schema.yml) | Schema generated from live session YAML. Windows-only. | `cargo iracing-sdk session schema live --output ./docs/reference/live-session-schema.yml` |
| [`docs/reference/live-variable-schema.yml`](docs/reference/live-variable-schema.yml) | Schema generated from live telemetry variables. Windows-only. | Generator deferred during CLI consolidation. |

## Getting Started

```bash
# Clone and enter the workspace
git clone https://github.com/racedirector/iracing.rs
cd iracing.rs

# Regenerate and verify deterministic telemetry fixtures when needed
cargo test-fixtures

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

| Command | Purpose |
| --- | --- |
| `cargo test-fixtures` | Generate, verify, and drift-check deterministic fixtures. |
| `cargo iracing-sdk telemetry convert --path <FILE.ibt> --format csv --output <FILE.csv>` | Convert all recorded telemetry frames to CSV; use `--format jsonl` for JSON Lines. |
| `cargo iracing-sdk telemetry record --format jsonl --output <FILE.jsonl>` | Record live telemetry until stream end or Ctrl+C (Windows). |
| `cargo iracing-sdk telemetry snapshot ibt --path <FILE.ibt> --index 0` | Capture one recorded telemetry frame. |
| `cargo iracing-sdk telemetry snapshot live` | Capture one live telemetry frame (Windows). |
| `cargo iracing-sdk session snapshot ibt --path <FILE.ibt>` | Extract session data from a recording. |
| `cargo iracing-sdk session schema type` | Emit the baseline session schema. |
| `cargo iracing-sdk session schema ibt --path <FILE.ibt>` | Generate session schema from a recording. |
| `cargo iracing-sdk session schema live` | Generate live session schema (Windows). |
| `cargo iracing-sdk headers layout --path <FILE.ibt>` | Inspect physical source regions and runtime frame fields in published header order; use `--format json` or `--format yaml` for structured output. |
| `cargo iracing-sdk variables ibt --path <FILE.ibt>` | Export variable metadata. |
| `cargo iracing-sdk broadcast --help` | Inspect simulator broadcast commands (Windows). |

## Development Notes

- **Platform gates**: Live shared-memory and broadcast subcommands are Windows-only. The consolidated executable also supports portable IBT commands; gate OS-specific implementations with `#[cfg(windows)]`.
- **Telemetry decoding**: Always use `VarData::decode_field` and related helpers; frame data is little-endian and manual decoding tends to drift from the authoritative implementation.
- **Session parsing**: Parse provider-supplied YAML with `SessionInfo::parse`. The telemetry session policies handle live version changes and parse IBT session data once.
- **Adapters**: `FrameAdapter::validate_layout` returns an `AdapterValidation` that should pre-resolve declaration-ordered field IDs and retain their originating layout; `adapt` must avoid schema map lookups for per-frame performance. The primary adapter surface is in `crates/iracing-sdk`.
- **Schema discovery**: When new fields appear, run `cargo iracing-sdk session discover ibt --path <FILE.ibt>` (or `discover live` on Windows) and incorporate the results back into `iracing-sdk` to improve typings.
- **Fixtures**: Integration tests use deterministic generated `.ibt` fixtures listed in `test-data/ibt/manifest.json` (see `iracing_sdk::test_utils`). `cargo test-fixtures` regenerates and verifies fixtures, then runs a scoped `git diff --exit-code` check for drift. After intentional profile changes, run `cargo test-fixtures check --no-drift-check`, review and stage the generated `.ibt`, YAML, and manifest artifacts, then run `cargo test-fixtures` as the clean-tree verification step after those changes are staged or committed.

## Testing

- `cargo test -p iracing-sdk --doc` and `RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps` duplicate the `Docs` CI job (doctests, docs, plus separate `cargo check` commands for SDK examples and CLI binaries when run manually).
- Use crate-specific invocations like `cargo test -p iracing-sdk -- types::tests::bitfield_constructor_works` to target individual tests.
- Benchmarks (`criterion`) require enabling the `benchmark` feature on the relevant crate, e.g. `cargo bench -p iracing-sdk --features benchmark`.
- Integration tests that rely on telemetry fixtures will fail fast with actionable messaging if generated fixtures are missing. Regenerate with `cargo test-fixtures generate`.

## Release Workflow

- Tag releases (`v*`) trigger the cargo-dist pipeline defined in `.github/workflows/release.yml`. It builds platform-specific artifacts and can host them back to GitHub Releases.
- Keep dist metadata (`[package.metadata.dist]` sections) aligned with any new binaries or feature gates, especially for Windows-only executables.

## Additional Resources

- Per-crate guidance lives alongside each package (`crates/*/AGENTS.md`). Start there for deep-dive development tips.
- Inspect the consolidated commands with `cargo iracing-sdk --help` and each subcommand's `--help`.
- Telemetry consumer examples reside under `examples/` in the respective crates; run them with `cargo run -p <crate> --example <name> -- --help` to inspect options.
- Release notes and packaging pointers live in `docs/releasing.md`.
