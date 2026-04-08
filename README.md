# iracing.rs

Rust workspace for working with iRacing telemetry: reading `.ibt` recordings, consuming live shared memory, streaming frames to higher-level adapters, generating schemas, and probing the simulation lifecycle.

## Executive Summary
- **Telemetry core (`iracing-sdk`)** — foundational library for decoding iRacing telemetry, encapsulating frame parsing, session YAML caching, and Windows shared-memory access.
- **Adapter layer (`iracing-sdk-adapter`)** — stream-oriented abstractions (`Provider`, `FrameAdapter`) that turn raw frames into typed views or lightweight projections.
- **Schema tooling (`iracing-sdk-codegen`)** — command-line utilities that emit JSON Schema/YAML representations of telemetry/session data for documentation and contract testing.
- **Simulation utilities (`iracing-simulation`)** — minimal HTTP probe for determining whether the iRacing sim is running, with configurable transports and examples.
- **Shared test helpers (`test-utils`)** — Git LFS-aware fixture discovery and test-data helpers used across the workspace.

The workspace targets cross-platform offline replay with Windows-specific features for live telemetry and broadcast commands. CI enforces doc and example correctness for `iracing-sdk`, and cargo-dist powers tag-driven releases.

## Workspace Packages

| Crate | Location | Highlights |
| --- | --- | --- |
| `iracing-sdk` | `crates/iracing-sdk` | `.ibt` reader (`IbtReader`), session YAML parser (`SessionInfoParser`), telemetry type system (`VariableSchema`, `VarData`), Windows shared-memory API (`WindowsConnection`), CLI bins for parsing/exporting telemetry. |
| `iracing-sdk-adapter` | `crates/iracing-sdk-adapter` | Streaming providers for disk/live telemetry, adapter contract that pre-computes field indices, `DynamicFrame` for exploratory access, examples that convert frames into user-defined structs. |
| `iracing-sdk-codegen` | `crates/iracing-sdk-codegen` | Schema generation binaries (disk/live telemetry, session data, car setup, primitives). Retains both `codegen` and `schema-discovery` features on `iracing-sdk`. |
| `iracing-simulation` | `crates/iracing-simulation` | Dependency-light HTTP client (`StdSimStatusClient`) and façade (`Simulation`) for the sim-status endpoint, plus examples using `reqwest`/`ureq`. |
| `test-utils` | `crates/test-utils` | Git-aware test-data path discovery and fixture checks that standardize missing-fixture messaging across integration tests. |

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

### Common Cargo Aliases
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
