# AGENTS.md

## Critical Commands

- `cargo test -p iracing-sdk --features test-utils --all-targets` runs the crate test suite, including fixture-backed integration tests; add `-- types::tests::bitfield_constructor_works` to laser in on a single test.
- `cargo test -p iracing-sdk --features benchmark --lib benchmarking::` runs retained acknowledgement/backpressure/session experiment tests; quality CI and `cargo xtask pre-push` run this explicitly.
- `cargo test -p iracing-sdk --doc` followed by `RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps` mirrors this crate's docs CI job.
- `cargo check -p iracing-sdk --examples` checks SDK examples; `cargo check -p iracing-sdk-cli --bins --all-features` checks relocated CLI binaries.
- Enable schema tools with `cargo build -p iracing-sdk-cli` when generating schema outputs.

## Key APIs & Layout

- `ibt::IbtFile` is the sole completed-recording model: `open` retains a read-only map, `from_bytes` owns bytes, and construction establishes immutable variable/session snapshots plus one shared `TelemetryLayout`. Inspect physical geometry through `physical_layout()` and semantic fields through `telemetry_layout()`. `frame(index)` and lazy `frames(start..end)` return `IbtFrame` with `usize` physical identity and the file's exact shared layout; `IbtFrame::into_packet()` is the SDK-owned recorded packet bridge with checked `u32` representability. `IbtFile::replay()` transfers ownership to `IbtReplay`; `file()`/`into_file()` borrow/recover the same file.
- Mapped IBT recordings must remain unchanged until the file or owning replay is dropped. Never mutate or truncate a live mapped test fixture; inject short reads through owned test sources instead.
- `ibt/`: recorded frames/ranges are cursor-free, indexed by zero-based `usize`, and located through `IbtLayout` in O(1). Empty ranges at EOF are valid; ends clamp to frame count, reversed ranges and starts beyond EOF fail. `IbtReplay` owns only sequential bounds/cursor/seek/EOF over the file, advances after successful reads, and has no clock or background subscription.
- `types/ibt/`: `IbtLayout` is the canonical physical geometry validated from `Header` and source length without I/O. Use its region/offset/frame extent accessors rather than recalculating geometry. It knows no telemetry field semantics. `IbtFile` rejects frames without variable metadata but supports zero-frame recordings with empty metadata/layout. Source lengths must fit `usize` (4 GiB files are rejected on 32-bit targets).
- `types/`: `TelemetryLayout`, `FieldLayout`, `VarData`, `FramePacket`, `DynamicFrame`, broadcast enums, incident helpers, and bitfield enums. Always decode telemetry via `VarData::decode_field` (little-endian) rather than manual slicing.
- `schema/session/`: `SessionInfo::parse` deserializes decoded YAML; `SessionInfo::try_from(SessionInfoBytes)` follows SDK decoding/sanitization. Recorded consumers decide when to parse their immutable snapshot; live telemetry policy tracks session revisions.
- Live schema discovery: use `WindowsConnection` for shared-memory access, `irsdk::{Header, VariableHeader}` for SDK wire layouts, and `TelemetryLayout` for variable metadata.
- `providers/`: source-independent `Provider` and Windows `LiveProvider` supply streaming packets/session YAML. Recorded consumers use file/replay directly; no recorded provider facade remains.
- `connections/`: `LiveConnection` exposes watch-backed latest snapshots/subscriptions and retains its portable non-Windows stub.
- `telemetry/`: source-independent frame loop and live delivery/session policies. `LatestDelivery` remains the live default. Recorded acknowledgement/backpressure/session experiments live only under feature-gated `benchmarking/`, with explicit unit-test coverage.
- `adapters/`: `FrameAdapter`, `AdapterValidation`, `FieldExtraction`, `LayoutProvider` support typed per-frame extraction.
- Live activation must validate the full fixed header before unchecked scalar reads. Preserve event ownership in blocking workers after async cancellation; private mapping/event tests run on Windows without the simulator.
- `windows/`: `WindowsConnection`, `WaitResult`, shared-memory connection code, and broadcast helpers. Keep everything behind `#[cfg(windows)]`.
- `../iracing-sdk-cli/src/`: consolidated CLI; session schemas and discovery enable the SDK features through its dependency. Standalone variable, primitive, and car-setup generators are deferred.
- `examples/`: cross-platform disk examples plus Windows live/broadcast examples.
- `tests/`: integration and derive macro regression tests.
- `benches/`: Criterion-compatible targets gated by `benchmark`; CodSpeed runs CPU targets, while delivery and IBT remain local wall-time Criterion.
- `SessionInfoBytes` owns exact captured session bytes; `sanitized_yaml()` exposes decoded, cleaned text without parsing or reserialization, and the internal `IRacingSessionString` also supplies cleanup before typed parsing. Keep source decoding and cleanup on this path.

- Use the re-exported `irsdk::VariableType` for telemetry metadata. Reject `ElementTypeCount` at input boundaries and use checked SDK byte widths; do not introduce synthetic integer storage kinds.
- `TelemetryLayout::try_from_headers` is the runtime-layout foundation. It owns published-order `FieldLayout` records and layout-local `FieldId` lookup; geometry lives only in the copyable `VariableRegion`. Preserve authoritative header order and accept bounded overlapping fields. `VarData` provides field-based scalar/array decoding. Providers and packets retain the shared layout; adapter slots retain only layout-local IDs.

## Platform & Feature Guardrails

- Gate actual shared-memory, live-provider, and Win32 broadcast transports with `#[cfg(windows)]`. Keep portable typed commands and the non-Windows `LiveConnection` builder stub available where the public API already promises them.
- Recorded replay is pull-driven and advances only on explicit calls; live delivery remains latest-wins. Do not impose subscription acknowledgement or live tick identity on recorded file access.

## Examples & Binaries

- `.cargo/config.toml` exposes aliases like `cargo iracing-sdk`; they map to bins in `iracing-sdk-cli`.
- Use `cargo iracing-sdk session schema type`, `cargo iracing-sdk session schema ibt --path ./session.ibt`, `cargo iracing-sdk session discover ibt --path ./session.ibt`, and `cargo iracing-sdk session snapshot ibt --path ./session.ibt` for session schemas, discovery, and snapshots; `schema live`, `discover live`, and `snapshot live` require Windows. Output defaults to stdout; use `--output <file>` for a file. The CLI dependency enables `codegen,schema-discovery`.
- Keep cross-platform examples (`ibt-read-frame`, `manual-frame-adapter`, `ibt-derive`, `enum-bitfields-ibt`) runnable on non-Windows machines.
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
