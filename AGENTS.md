# AGENTS.md

Compact guidance for future agent sessions working in this repo. For durable
design context, start with `docs/architecture/README.md`; keep this file focused
on commands, boundaries, and easy-to-miss constraints.

## Critical Commands

- `cargo build --workspace` for workspace sanity; release builds defer to cargo-dist.
- `cargo test --workspace --all-targets` hits every crate; scope with `cargo test -p <crate>` or `cargo test -p iracing-sdk -- types::tests::bitfield_constructor_works` when debugging.
- `python scripts/check_test_fixtures.py` regenerates deterministic fixtures, verifies their manifest/bytes, and fails on git drift. Use `--no-drift-check` only when intentionally updating fixtures.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings` are the formatting/lint gates.
- For docs-touching crate changes, run the matching docs CI commands: `cargo test -p <crate> --doc`, `RUSTDOCFLAGS="-D warnings" cargo doc -p <crate> --no-deps`, and `cargo check -p <crate> --examples --bins`.
- Codegen binaries require `cargo build -p iracing-sdk --features codegen,schema-discovery` when you need schema outputs.

## Quality Gates

- Before committing or pushing, run and pass the checks that match `.github/workflows/quality.yml`: fixture verification, formatting, clippy, and workspace tests.
- When changes touch docs or public APIs for `iracing-sdk`, `iracing-sdk-derive`, or `iracing-simulation`, also run the crate-specific doctest, docs-as-warnings, and examples/bins checks from `.github/workflows/docs.yml`.
- CI runs the main quality gate on both Ubuntu and Windows. Local runs on one OS are useful, but do not ignore platform-specific failures surfaced by the other CI runner.

## Test Data

- Integration tests use deterministic generated `.ibt` fixtures listed in `test-data/ibt/manifest.json`; run `python3 scripts/check_test_fixtures.py` after changing fixture profiles.
- Use helpers from `crates/test-utils` (`require_ibt_fixtures`, `require_named_ibt_fixture`, `require_smallest_ibt_fixture`) instead of hardcoded paths so missing-fixture failures stay consistent.

## Workspace Map

- `crates/iracing-sdk`: low-level `.ibt` reader, schema/session parsing, provider and connection layers, telemetry delivery/session policies, typed adapters, and Windows shared-memory access. Keep the platform-neutral live connection stub and typed command data portable; gate actual Win32/shared-memory transports with `#[cfg(windows)]`.
- `crates/iracing-sdk-derive`: derive macros re-exported by `iracing-sdk` behind the `derive` feature.
- `crates/iracing-simulation`: portable HTTP status probe plus Windows-only process enumeration.
- `crates/iracing-broadcast-grpc-service`: generated cross-platform protobuf/tonic surface plus a Windows-only, layered command-and-observation service.
- `crates/test-utils`: shared fixture plumbing and path discovery; lean on it for integration tests instead of reinventing file lookups.
- `examples/*`: publish-disabled workspace applications that exercise the crates as downstream users would.

## Package-Specific Guidance

- Check for a nested `AGENTS.md` before editing a crate. Package guidance exists for `iracing-sdk`, `iracing-simulation`, `iracing-broadcast-grpc-service`, and `test-utils`.
- For the broadcast gRPC service, read `crates/iracing-broadcast-grpc-service/docs/architecture.md` before changing protocol, server, client, response semantics, platform support, or operational behavior.

## Patterns & Gotchas

- Frame extraction is little-endian; always rely on `VarData::from_bytes` rather than manual decoding to avoid drift.
- `SessionInfoParser` offers version-keyed caching for memory-backed callers. The telemetry task has separate live and IBT session policies; preserve their documented retry, ordering, and EOF semantics instead of adding a second per-frame parser.
- Live providers and Win32 transports compile only on Windows, while `LiveConnection` retains a portable stub and broadcast command types remain portable. Gate at the narrowest OS-dependent boundary.
- Do not assume recorded connection delivery is lossless. `OnDemandDelivery` exists, but `Telemetry::spawn_ibt` currently replaces only the session policy and still inherits latest-value delivery; finish and test that wiring before documenting replay as demand-driven.

## CI & Release

- `quality.yml` runs fixture verification, formatting, clippy, and workspace tests on pull requests and pushes to `main`.
- `docs.yml` runs doctests, docs with warnings as errors, and `cargo check` for examples/bins for touched documentation crates; run those targets locally before opening PRs.
- Releases are tag-driven (`v*`) via `cargo dist`; keep version bumps and dist metadata coordinated when preparing a release.
