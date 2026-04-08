# AGENTS.md

Guidance for OpenCode sessions inside `crates/iracing-sdk-adapter`.

## Critical Commands
- `cargo test -p iracing-sdk-adapter --all-targets` is the default regression suite.
- Spot-check adapters with `cargo test -p iracing-sdk-adapter -- adapters::tests::...`.
- `cargo bench -p iracing-sdk-adapter --features benchmark` enables the `frame_construction` benchmark.

## Core Concepts
- `Provider` trait exposes `next_frame()` and `session_yaml()`. `providers::ibt::IbtProvider` is cross-platform; `providers::live::LiveProvider` stays `#[cfg(windows)]`.
- `FrameAdapter::validate_schema` must pre-compute every lookup (build an `AdapterValidation` with `FieldExtraction` indices). `FrameAdapter::adapt` should only use those cached offsets—no schema map walks at runtime.
- `DynamicFrame` is for exploratory usage and tolerates by-name lookups; don’t use it on hot paths where adapters are expected.
- `SchemaProvider` exists so adapters and consumers can share schema references without cloning.

## Platform & Feature Notes
- Keep all live telemetry paths gated for Windows and mirror any new gates in `providers/mod.rs`.
- The crate expects `iracing-sdk` features `codegen`/`schema-discovery` to be off by default; only enable them per call site when necessary.

## Testing & Fixtures
- Integration tests depend on `.ibt` fixtures via `crates/test-utils`; prefer `require_ibt_fixtures()` and friends.
- When adding provider tests, use the helpers to keep missing-fixture errors consistent (`FIXTURE_INSTALL_GUIDANCE`).

## Examples
- CLI demos live under `examples/` (`disk-position`, `live-position`, `enum-bitfields-live`). Run them with `cargo run -p iracing-sdk-adapter --example <name> -- ...`; Windows examples should compile-gate cleanly on other platforms.
