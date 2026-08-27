# AGENTS.md

## Critical Commands

- `cargo test -p iracing-sdk --all-targets` runs the crate test suite; add `-- types::tests::bitfield_constructor_works` to laser in on a single test.
- `cargo test -p iracing-sdk --doc` followed by `RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps` mirrors this crate's docs CI job.
- `cargo check -p iracing-sdk --examples --bins` catches example/bin drift early.
- Enable schema tools with `cargo build -p iracing-sdk --features codegen,schema-discovery` when generating schema outputs.

## Key APIs & Layout

- `reader/`: source-neutral positioned access and header-directed snapshots; `reader::ibt::IbtRecording` owns validated `.ibt` layout while `IbtReader` adds cursor semantics.
- `types/`: `VariableSchema`, `VariableInfo`, `VarData`, `FramePacket`, `DynamicFrame`, broadcast enums, incident helpers, and bitfield enums. Always decode telemetry via `VarData::from_bytes` (little-endian) rather than manual slicing.
- `types/session/`: typed session YAML model. Readers return owned raw snapshots; providers and tools clean YAML with `yaml_utils` before `SessionInfo::parse`.
- `types/irsdk/`: literal shared live/IBT wire definitions; `types/schema.rs`: source-neutral variable metadata and schema construction.
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

## Refactoring and Failure Visibility

- During structural refactors, do not introduce compatibility aliases, re-exports, adapters, or other build-preserving abstractions unless the user explicitly requests them.
- Treat compiler failures as useful architectural evidence: perform the requested move or duplication, compile, categorize the exposed dependency breakages, and bring decisions about those boundaries back to the user before resolving them.
- Do not assume a green build is the correct intermediate result. Preserve intentional refactor-point failures when the user is reviewing ownership, public API boundaries, or dependency direction.
- Keep literal SDK wire definitions in `types/irsdk/` meaningful and internally explicit; do not conceal dependencies on their previous locations through compatibility names.

## Naming

- Prefer complete, descriptive Rust identifiers over abbreviations, including when the upstream SDK uses abbreviated C names. For example, use `ReplayPositionMode`, `TrackLocation`, and `PitServiceFlags`, and document their mappings to `irsdk_RpyPosMode`, `irsdk_TrkLoc`, and `irsdk_PitSvFlags`.
- Do not shorten public names to save typing; IDE completion handles long identifiers, while abbreviated names make discovery and review more ambiguous.

## Examples & Binaries

- `.cargo/config.toml` exposes aliases like `cargo ibt-to-csv`, `cargo live-session-parser`, `cargo broadcast-cli`; they map to bins in this crate.
- Keep cross-platform examples (`disk-position`, `adapter-disk-position`, `enum-bitfields-disk`) runnable on non-Windows machines.
- Keep adapter examples importing from `iracing_sdk`; derive examples should rely on the `derive` feature re-export from this crate.

## Testing & Fixtures

- Integration tests rely on `.ibt` fixtures from `test-data/ibt/`; use helpers in `test_utils` (`require_named_ibt_fixture`, `require_smallest_ibt_fixture`) instead of hard-coded paths.
- For hand-built schemas, session data, frames, and benchmark inputs, start with the generated catalog in `../../docs/reference/README.md` instead of guessing iRacing names or shapes. Preserve the `frame_size`, offsets, types, and counts from one disk/live variable snapshot as a coherent layout; consult `primitives-schema.yml` for enum/bitflag domains.
- Benchmarks require `cargo bench -p iracing-sdk --features benchmark`.
