# AGENTS.md

Compact context for OpenCode sessions working in `crates/iracing-sdk`.

## Critical Commands
- `cargo test -p iracing-sdk --all-targets` runs the crate test suite; add `-- types::tests::bitfield_constructor_works` to laser in on a single test.
- `cargo test -p iracing-sdk --doc` followed by `RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps` mirrors the CI docs workflow.
- `cargo check -p iracing-sdk --examples --bins` catches example/bin drift early.
- Enable schema tools with `cargo build -p iracing-sdk --features codegen,schema-discovery` (the codegen crate expects both flags).

## Key APIs & Layout
- `ibt/`: `IbtReader` iterates `.ibt` telemetry; rely on `VarHeaders` metadata instead of re-parsing frame bytes.
- `types/`: `VariableSchema`, `VariableInfo`, `VarData`, and bitfield enums; always decode via `VarData::from_bytes` (little-endian) rather than manual slicing.
- `schema/session`: `SessionInfoParser` caches YAML; only re-parse when `session_version` changes.
- `providers/`: `Provider`, `IbtProvider`, and `LiveProvider` stream `FramePacket` values plus session YAML.
- `frame/`: `FramePacket` and `DynamicFrame` provide raw frame access and ad-hoc lookups.
- `adapters/`: `FrameAdapter`, `AdapterValidation`, `FieldExtraction`, `DefaultValue`, and `SchemaProvider` support typed per-frame extraction.
- `windows/`: `WindowsConnection`, `WaitResult`, broadcast helpers—everything behind `#[cfg(windows)]`.
- `yaml_utils`: cleans iRacing’s malformed YAML before parsing; use it instead of custom scrubbing.

## Platform & Feature Guardrails
- Live telemetry and broadcast APIs must stay `#[cfg(windows)]`; add matching targets in `package.metadata.dist.bin.*.targets` when adding bins.
- Tokio support is optional; gate async wait paths behind the `tokio` feature.

## Examples & Binaries
- `.cargo/config.toml` exposes aliases like `cargo ibt-to-csv`, `cargo live-session-parser`, `cargo broadcast-cli`; they map to bins in this crate.
- Keep cross-platform examples (`disk-position`, `adapter_disk_position`, `enum-bitfields-disk`) runnable on non-Windows machines.
- Keep adapter examples importing from `iracing_sdk`, not `iracing_sdk_adapter`.

## Testing & Fixtures
- Integration tests rely on `.ibt` fixtures from `test-data/ibt/`; use helpers in `crates/test-utils` (`require_named_ibt_fixture`, `require_smallest_ibt_fixture`) instead of hard-coded paths.
- Benchmarks require `cargo bench -p iracing-sdk --features benchmark`.
