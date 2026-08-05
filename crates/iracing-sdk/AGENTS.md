# AGENTS.md

## Critical Commands

- `cargo test -p iracing-sdk --all-targets` runs the crate test suite; add `-- types::tests::bitfield_constructor_works` to laser in on a single test.
- `cargo test -p iracing-sdk --doc` followed by `RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps` mirrors this crate's docs CI job.
- `cargo check -p iracing-sdk --examples --bins` catches example/bin drift early.
- Enable schema tools with `cargo build -p iracing-sdk --features codegen,schema-discovery` when generating schema outputs.

## Key APIs & Layout

- `ibt/`: `IbtReader` iterates `.ibt` telemetry; rely on `VariableSchema` and `VariableInfo` metadata instead of re-parsing frame bytes.
- `types/`: `VariableSchema`, `VariableInfo`, `VarData`, `FramePacket`, `DynamicFrame`, broadcast enums, incident helpers, and bitfield enums. Always decode telemetry via `VarData::from_bytes` (little-endian) rather than manual slicing.
- `schema/session/`: `SessionInfoParser` caches YAML; only re-parse when `session_version` changes.
- `schema/header.rs` and `schema/variables.rs`: Windows-only live schema discovery for shared-memory headers and variable definitions.
- `providers/`: `Provider`, `IbtProvider`, and `LiveProvider` stream `FramePacket` values plus session YAML.
- `connections/`: higher-level `IbtConnection` and `LiveConnection` subscription APIs. `IbtConnection` coordinates one shared cursor across acknowledged subscribers; `LiveConnection` exposes watch-backed latest snapshots.
- `telemetry/`: shared frame-read loop plus explicit delivery and session policies. `LatestDelivery` is the live default, while `Telemetry::spawn_ibt` selects `OnDemandDelivery`.
- `adapters/`: `FrameAdapter`, `AdapterValidation`, `FieldExtraction`, `DefaultValue`, and `SchemaProvider` support typed per-frame extraction.
- `windows/`: `WindowsConnection`, `WaitResult`, shared-memory connection code, and broadcast helpers. Keep everything behind `#[cfg(windows)]`.
- `src/bin/`: CLI and schema-generation binaries; codegen binaries require the `codegen` feature, and discovery overlays require `schema-discovery`.
- `examples/`: cross-platform disk examples plus Windows live/broadcast examples.
- `tests/`: integration and derive macro regression tests.
- `benches/`: Criterion benchmarks gated by the `benchmark` feature.
- `yaml_utils`: cleans iRacing's malformed YAML before parsing; use it instead of custom scrubbing.

## Platform & Feature Guardrails

- Gate actual shared-memory, live-provider, and Win32 broadcast transports with `#[cfg(windows)]`. Keep portable typed commands and the non-Windows `LiveConnection` builder stub available where the public API already promises them.
- Recorded and live sources have different delivery semantics. IBT replay is explicitly started and advances one shared cursor only after every active subscription asks for its next item; live delivery remains latest-wins.
- Tokio is a target-specific internal dependency: native targets use full Tokio, while `wasm32` builds are limited to Tokio's WASM-safe subset.
- Only gate APIs that require incompatible Tokio runtime behavior; `tokio::sync` usage can stay in shared code.

## Examples & Binaries

- `.cargo/config.toml` exposes aliases like `cargo ibt-to-csv`, `cargo live-session-parser`, `cargo broadcast-cli`; they map to bins in this crate.
- Keep cross-platform examples (`disk-position`, `adapter-disk-position`, `enum-bitfields-disk`) runnable on non-Windows machines.
- Keep adapter examples importing from `iracing_sdk`; derive examples should rely on the `derive` feature re-export from this crate.

## Testing & Fixtures

- Integration tests rely on `.ibt` fixtures from `test-data/ibt/`; use helpers in `test_utils` (`require_named_ibt_fixture`, `require_smallest_ibt_fixture`) instead of hard-coded paths.
- For hand-built schemas, session data, frames, and benchmark inputs, start with the generated catalog in `../../docs/reference/README.md` instead of guessing iRacing names or shapes. Preserve the `frame_size`, offsets, types, and counts from one disk/live variable snapshot as a coherent layout; consult `primitives-schema.yml` for enum/bitflag domains.
- Benchmarks require `cargo bench -p iracing-sdk --features benchmark`.
