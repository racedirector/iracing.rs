# AGENTS.md

## Critical Commands

- `cargo test -p iracing-sdk --all-targets` runs the crate test suite; add `-- types::tests::bitfield_constructor_works` to laser in on a single test.
- `cargo test -p iracing-sdk --doc` followed by `RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps` mirrors this crate's docs CI job.
- `cargo check -p iracing-sdk --examples` checks SDK examples; `cargo check -p iracing-sdk-cli --bins --all-features` checks relocated CLI binaries.
- Enable schema tools with `cargo build -p iracing-sdk-cli` when generating schema outputs.

## Key APIs & Layout

- Mapped IBT recordings must remain unchanged until the reader/provider/connection is dropped. Never mutate or truncate a live mapped test fixture; inject short reads through owned test sources instead.
- `ibt/`: `IbtReader` provides indexed raw frame reads and fresh metadata snapshots. `IbtProvider` owns schema validation and sequential replay starting at frame zero; rely on `VariableSchema` and `VariableInfo` instead of re-parsing frame bytes.
- `types/ibt/`: `IbtLayout` validates physical byte geometry from `Header` and source length without I/O. `MetadataRegions` lives in `types/regions/`; `IbtReader` delegates all geometry to the layout. The reader has no schema, session cache, or logical replay cursor. Provider construction rejects frames without variable headers but accepts empty recordings without metadata. Source lengths must fit `usize` (4 GiB files are rejected on 32-bit targets).
- `types/`: `VariableSchema`, `VariableInfo`, `VarData`, `FramePacket`, `DynamicFrame`, broadcast enums, incident helpers, and bitfield enums. Always decode telemetry via `VarData::from_bytes` (little-endian) rather than manual slicing.
- `schema/session/`: `SessionInfo::parse` deserializes decoded session YAML; the live telemetry session policy tracks `session_version`, while IBT parses its session once.
- Live schema discovery: use `WindowsConnection` for shared-memory access, `irsdk::{Header, VariableHeader}` for SDK wire layouts, and `VariableSchema` for variable metadata.
- `providers/`: `Provider`, `IbtProvider`, and `LiveProvider` stream `FramePacket` values plus session YAML.
- `connections/`: higher-level `IbtConnection` and `LiveConnection` subscription APIs. `IbtConnection` coordinates one shared cursor across acknowledged subscribers; `LiveConnection` exposes watch-backed latest snapshots.
- `telemetry/`: shared frame-read loop plus explicit delivery and session policies. `LatestDelivery` is the live default, while `Telemetry::spawn_ibt` selects `OnDemandDelivery`.
- `adapters/`: `FrameAdapter`, `AdapterValidation`, `FieldExtraction`, `DefaultValue`, and `SchemaProvider` support typed per-frame extraction.
- `windows/`: `WindowsConnection`, `WaitResult`, shared-memory connection code, and broadcast helpers. Keep everything behind `#[cfg(windows)]`.
- `../iracing-sdk-cli/src/`: consolidated CLI; session schemas and discovery enable the SDK features through its dependency. Standalone variable, primitive, and car-setup generators are deferred.
- `examples/`: cross-platform disk examples plus Windows live/broadcast examples.
- `tests/`: integration and derive macro regression tests.
- `benches/`: Criterion-compatible targets gated by `benchmark`; CodSpeed runs CPU targets, while delivery and IBT remain local wall-time Criterion.
- `SessionInfoBuffer` bounds and decodes captured session bytes; `IRacingSessionString` removes invalid control characters before parsing. Keep that cleanup in the source path.

- Use the re-exported `irsdk::VariableType` for telemetry metadata. Reject `ElementTypeCount` at input boundaries and use checked SDK byte widths; do not introduce synthetic integer storage kinds.

## Platform & Feature Guardrails

- Gate actual shared-memory, live-provider, and Win32 broadcast transports with `#[cfg(windows)]`. Keep portable typed commands and the non-Windows `LiveConnection` builder stub available where the public API already promises them.
- Recorded and live sources have different delivery semantics. IBT replay is explicitly started and advances one shared cursor only after every active subscription asks for its next item; live delivery remains latest-wins.

## Examples & Binaries

- `.cargo/config.toml` exposes aliases like `cargo iracing-sdk`; they map to bins in `iracing-sdk-cli`.
- Use `cargo iracing-sdk session schema type`, `cargo iracing-sdk session schema ibt --path ./session.ibt`, `cargo iracing-sdk session discover ibt --path ./session.ibt`, and `cargo iracing-sdk session snapshot ibt --path ./session.ibt` for session schemas, discovery, and snapshots; `schema live`, `discover live`, and `snapshot live` require Windows. Output defaults to stdout; use `--output <file>` for a file. The CLI dependency enables `codegen,schema-discovery`.
- Keep cross-platform examples (`ibt-read-frame`, `manual-frame-adapter`, `ibt-subscribe`, `enum-bitfields-ibt`) runnable on non-Windows machines.
- Keep adapter examples importing from `iracing_sdk`; derive examples should rely on the `derive` feature re-export from this crate.

## Testing & Fixtures

- Integration tests rely on `.ibt` fixtures from `test-data/ibt/`; use helpers in `test_utils` (`require_named_ibt_fixture`, `require_smallest_ibt_fixture`) instead of hard-coded paths.
- For hand-built schemas, session data, frames, and benchmark inputs, start with the generated catalog in `../../docs/reference/README.md` instead of guessing iRacing names or shapes. Preserve the `frame_size`, offsets, types, and counts from one disk/live variable snapshot as a coherent layout; consult `primitives-schema.yml` for enum/bitflag domains.
- Benchmarks require `cargo bench -p iracing-sdk --features benchmark`.

- Downstream fixture tests can enable the SDK's `test-utils` feature without enabling benchmark targets.
