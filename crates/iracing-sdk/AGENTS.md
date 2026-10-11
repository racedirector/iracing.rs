# AGENTS.md

## Critical Commands

- `cargo test -p iracing-sdk --all-targets` runs the crate test suite; add `-- types::tests::bitfield_constructor_works` to laser in on a single test.
- `cargo test -p iracing-sdk --doc` followed by `RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps` mirrors this crate's docs CI job.
- `cargo check -p iracing-sdk --examples` checks SDK examples; `cargo check -p iracing-sdk-cli --bins --all-features` checks relocated CLI binaries.
- Enable schema tools with `cargo build -p iracing-sdk-cli` when generating schema outputs.

## Key APIs & Layout

- `ibt::IbtFile` is the owning completed-recording model: `open` retains a read-only map, `from_bytes` owns bytes, and construction establishes immutable variable/session snapshots plus one shared `TelemetryLayout`. Inspect physical geometry through `physical_layout()` and semantic fields through `telemetry_layout()`. `frame(index)` and lazy `frames(start..end)` return `IbtFrame` with `usize` physical identity and the file's exact shared layout; `IbtFrame::into_packet()` is the SDK-owned recorded packet bridge with checked `u32` representability. Replay/provider/CLI consumers still use `IbtReader` during the #299 migration; #302–#305 move those responsibilities and remove the old topology.
- Mapped IBT recordings must remain unchanged until the reader/provider/connection is dropped. Never mutate or truncate a live mapped test fixture; inject short reads through owned test sources instead.
- `ibt/`: `IbtReader` provides indexed raw frame reads, validated half-open `frames(start..end)` pull traversal with explicit recorded indices, and fresh metadata snapshots. Coordinates are `usize` record indices, not live ticks; empty ranges at EOF are valid. Range ends clamp to frame count; reversed ranges and starts beyond EOF are errors. `IbtReplay` owns sequential cursor/bounds/seek/EOF over the reader; `IbtProvider` owns schema validation and delegates traversal to replay, starting at zero via `from_reader` or preserving bounds/position via `from_replay`; rely on `TelemetryLayout` and `FieldLayout` instead of re-parsing frame bytes.
- `types/ibt/`: `IbtLayout` is the canonical physical IBT description, validated from `Header` and source length without I/O. Use its source-length, fixed-region, metadata, and frame accessors for runtime access and inspection; do not recalculate geometry in tooling. `MetadataRegions` lives in `types/regions/`; `IbtReader` delegates all geometry to the layout. The reader has no schema, session cache, or logical replay cursor. Provider construction rejects frames without variable headers but accepts empty recordings without metadata. Source lengths must fit `usize` (4 GiB files are rejected on 32-bit targets).
- `types/`: `TelemetryLayout`, `FieldLayout`, `VarData`, `FramePacket`, `DynamicFrame`, broadcast enums, incident helpers, and bitfield enums. Always decode telemetry via `VarData::decode_field` (little-endian) rather than manual slicing.
- `schema/session/`: `SessionInfo::parse` deserializes decoded session YAML; the live telemetry session policy tracks `session_version`, while IBT parses its session once.
- Live schema discovery: use `WindowsConnection` for shared-memory access, `irsdk::{Header, VariableHeader}` for SDK wire layouts, and `TelemetryLayout` for variable metadata.
- `providers/`: `Provider`, `IbtProvider`, and `LiveProvider` stream `FramePacket` values plus session YAML.
- `connections/`: higher-level `IbtConnection` and `LiveConnection` subscription APIs. `IbtConnection` coordinates one shared cursor across acknowledged subscribers; `LiveConnection` exposes watch-backed latest snapshots.
- `telemetry/`: shared frame-read loop plus explicit delivery and session policies. `LatestDelivery` is the live default, while `Telemetry::spawn_ibt` selects `OnDemandDelivery`.
- `adapters/`: `FrameAdapter`, `AdapterValidation`, `FieldExtraction`, `LayoutProvider` support typed per-frame extraction.
- Live activation must validate the full fixed header before unchecked scalar reads. Preserve event ownership in blocking workers after async cancellation; private mapping/event tests run on Windows without the simulator.
- `windows/`: `WindowsConnection`, `WaitResult`, shared-memory connection code, and broadcast helpers. Keep everything behind `#[cfg(windows)]`.
- `../iracing-sdk-cli/src/`: consolidated CLI; session schemas and discovery enable the SDK features through its dependency. Standalone variable, primitive, and car-setup generators are deferred.
- `examples/`: cross-platform disk examples plus Windows live/broadcast examples.
- `tests/`: integration and derive macro regression tests.
- `benches/`: Criterion-compatible targets gated by `benchmark`; CodSpeed runs CPU targets, while delivery and IBT remain local wall-time Criterion.
- `SessionInfoBuffer` bounds and decodes captured session bytes; `IRacingSessionString` removes invalid control characters before parsing. Keep that cleanup in the source path.

- Use the re-exported `irsdk::VariableType` for telemetry metadata. Reject `ElementTypeCount` at input boundaries and use checked SDK byte widths; do not introduce synthetic integer storage kinds.
- `TelemetryLayout::try_from_headers` is the runtime-layout foundation. It owns published-order `FieldLayout` records and layout-local `FieldId` lookup; geometry lives only in the copyable `VariableRegion`. Preserve authoritative header order and accept bounded overlapping fields. `VarData` provides field-based scalar/array decoding. Providers and packets retain the shared layout; adapter slots retain only layout-local IDs.

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

## Soundness review routing

For proof-oriented unsafe/invariant audits, compose `rust-soundness-review` and
`rust-unsafe-ffi`. Read `docs/architecture/unsafe-boundaries.md` from the workspace
root; trace current producers and consumers and keep unimplemented proposals
separate from current-source conclusions.
