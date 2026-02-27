# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace --all-targets

# Run tests for a single crate
cargo test -p iracing-sdk
cargo test -p iracing-sdk-adapter

# Run a single test by name
cargo test -p iracing-sdk -- types::tests::bitfield_constructor_works

# Run doctests
cargo test -p iracing-sdk --doc

# Check formatting
cargo fmt --all -- --check

# Lint (treat warnings as errors)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Build docs (with warnings as errors, as in CI)
RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps

# Build with a specific feature
cargo build -p iracing-sdk --features codegen
```

### Test Fixtures (Git LFS)

Integration tests that read `.ibt` telemetry files require Git LFS fixtures:

```bash
git lfs install
git lfs pull
```

Fixtures live under `test-data/ibt/`. Tests that need them use helpers from `crates/test-utils` (`require_ibt_fixtures`, `require_named_ibt_fixture`, etc.). If fixtures are missing, tests fail with a clear message pointing to Git LFS.

## Workspace Architecture

This is a Cargo workspace (`resolver = "3"`, edition `2024`) with five crates:

### `crates/iracing-sdk` — Core parsing library

The foundation crate. Cross-platform for `.ibt` replay; Windows-only for live shared-memory.

Key modules:
- `ibt/` — `IbtReader`: reads `.ibt` telemetry files frame-by-frame (cross-platform)
- `types/` — `VariableSchema`, `VariableInfo`, `VariableType`, `VarData` trait, `BitField`, iRacing enums/flags
- `schema/session` — `SessionInfo` and `SessionInfoParser`: YAML parsing and caching
- `windows/` — `WindowsConnection`, `WaitResult`, `Broadcast`, `BroadcastCommand` (Windows only, gated with `#[cfg(windows)]`)
- `yaml_utils` — helpers for cleaning up iRacing's non-standard YAML output

Feature flags: `codegen` (schema generation), `schema-discovery` (unknown-field discovery), `tokio` (async wait for Windows), `benchmark`.

### `crates/iracing-sdk-adapter` — Higher-level streaming layer

Wraps `iracing-sdk` with a stream-oriented abstraction for consuming telemetry.

Key types:
- `FramePacket` — fundamental data unit: `Arc<[u8]>` data + tick + session version + `Arc<VariableSchema>`
- `Provider` trait — `next_frame() -> Option<FramePacket>` + `session_yaml(version)`
- `IbtProvider` / `LiveProvider` (Windows only) — implement `Provider` for `.ibt` replay and live telemetry
- `FrameAdapter` trait — dual-phase adapter pattern: `validate_schema` (connection-time, builds extraction plan) and `adapt` (runtime, zero-HashMap extraction)
- `AdapterValidation` / `FieldExtraction` — the extraction plan built during validation
- `DynamicFrame` — by-name variable lookup without a typed struct; good for exploration, not hot paths
- `SchemaProvider` — trait for types that expose a `VariableSchema`

### `crates/iracing-sdk-codegen` — Schema generation binaries

Generates JSON/YAML schemas for iRacing data structures. Depends on `iracing-sdk` with `codegen` and `schema-discovery` features. All binaries are Windows-only for distribution. Binaries: `session-schema`, `disk-variable-schema`, `live-variable-schema`, `disk-session-schema`, `live-session-schema`, `car-setup-schema`, `iracing-primitives-schema`.

### `crates/iracing-simulation` — Simulation status helper

Thin crate exposing a `Simulation` type for querying the iRacing sim's running status via HTTP.

### `crates/test-utils` — Shared test helpers

Provides `find_git_repository_root`, `get_test_data_dir`, `require_ibt_fixtures`, `require_named_ibt_fixture`, `require_smallest_ibt_fixture`. Use these in integration tests to resolve paths to `test-data/ibt/*.ibt` fixtures consistently across all crates.

## Key Patterns

### Platform gating

Live telemetry (Windows shared memory) is gated with `#[cfg(windows)]` throughout. The `windows/` module in `iracing-sdk` and the `LiveProvider` in `iracing-sdk-adapter` are compiled only on Windows. Binaries that use live telemetry declare `targets = ["x86_64-pc-windows-msvc"]` in `Cargo.toml`.

### FrameAdapter dual-phase pattern

`FrameAdapter::validate_schema` runs once at connection time and returns an `AdapterValidation` containing a pre-resolved `Vec<FieldExtraction>`. `FrameAdapter::adapt` uses this plan to extract fields with direct index access, avoiding per-frame HashMap lookups. For exploration use `DynamicFrame`; for production hot paths implement `FrameAdapter`.

### `VarData` trait

Binary telemetry is extracted via `T::from_bytes(&frame_bytes, &variable_info)`. All primitive types (`f32`, `i32`, `u32`, `f64`, `bool`, `Vec<T>`, `BitField`) implement `VarData`. All iRacing values are little-endian.

### Session YAML

iRacing session info is embedded as YAML inside `.ibt` files and in live shared memory. `yaml_utils` cleans up iRacing's non-standard YAML before parsing. `SessionInfoParser` caches the parsed result and only re-parses when `session_version` changes.

## CI

`.github/workflows/docs.yml` — runs doctests, builds docs with `RUSTDOCFLAGS="-D warnings"`, and checks examples/bins. Triggers on changes to `crates/iracing-sdk/**`.

`.github/workflows/release.yml` — cargo-dist release pipeline. Distribution targets: `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`. Homebrew tap: `racedirector/homebrew-tap`.
