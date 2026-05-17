# AGENTS.md

Compact guidance for future OpenCode sessions working in this repo.

## Critical Commands

- `cargo build --workspace` for workspace sanity; release builds defer to cargo-dist.
- `cargo test --workspace --all-targets` hits every crate; scope with `cargo test -p <crate>` or `cargo test -p iracing-sdk -- types::tests::bitfield_constructor_works` when debugging.
- `python scripts/check_test_fixtures.py` verifies generated `.ibt` fixtures before tests.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings` are the formatting/lint gates.
- `cargo check -p iracing-sdk --lib --target wasm32-unknown-unknown --all-features` mirrors the wasm compatibility gate; install the target with `rustup target add wasm32-unknown-unknown` if needed.
- For docs-touching crate changes, run the matching docs CI commands: `cargo test -p <crate> --doc`, `RUSTDOCFLAGS="-D warnings" cargo doc -p <crate> --no-deps`, and `cargo check -p <crate> --examples --bins`.
- Codegen binaries require `cargo build -p iracing-sdk --features codegen,schema-discovery` when you need schema outputs.

## Quality Gates

- Before committing or pushing, run and pass the checks that match `.github/workflows/quality.yml`: fixture verification, formatting, clippy, workspace tests, and the wasm compatibility check.
- When changes touch docs or public APIs for `iracing-sdk`, `iracing-sdk-derive`, or `iracing-simulation`, also run the crate-specific doctest, docs-as-warnings, and examples/bins checks from `.github/workflows/docs.yml`.
- CI runs the main quality gate on both Ubuntu and Windows. Local runs on one OS are useful, but do not ignore platform-specific failures surfaced by the other CI runner.

## Test Data

- Integration tests use deterministic generated `.ibt` fixtures listed in `test-data/ibt/manifest.json`; run `python3 scripts/check_test_fixtures.py` after changing fixture profiles.
- Use helpers from `crates/test-utils` (`require_ibt_fixtures`, `require_named_ibt_fixture`, `require_smallest_ibt_fixture`) instead of hardcoded paths so missing-fixture failures stay consistent.

## Workspace Map

- `crates/iracing-sdk`: low-level `.ibt` reader, session YAML parser, streaming adapter APIs, and Windows shared-memory access. All live-telemetry additions must stay behind `#[cfg(windows)]` and keep binaries' `package.metadata.dist.bin.*.targets` in sync.
- `crates/iracing-sdk-derive`: derive macros re-exported by `iracing-sdk` behind the `derive` feature.
- `crates/iracing-simulation`: minimal HTTP probe for the sim; no Windows gating.
- `crates/iracing-broadcast-grpc-service`: gRPC broadcast service and generated proto bindings.
- `crates/test-utils`: shared fixture plumbing and path discovery; lean on it for integration tests instead of reinventing file lookups.
- `examples/*`: workspace example applications that consume the crates as downstream users would.

## Patterns & Gotchas

- Frame extraction is little-endian; always rely on `VarData::from_bytes` rather than manual decoding to avoid drift.
- Session YAML parsing is cached via `SessionInfoParser`; reuse it when adding code so you don't reparse on every frame.
- Live telemetry only compiles on Windows. Non-Windows builds will skip those modules, so gate new APIs and tests accordingly to preserve cross-platform builds.

## CI & Release

- `quality.yml` runs fixture verification, formatting, clippy, workspace tests, and wasm compatibility on pull requests and pushes to `main`.
- `docs.yml` runs doctests, docs with warnings as errors, and `cargo check` for examples/bins for touched documentation crates; run those targets locally before opening PRs.
- Releases are tag-driven (`v*`) via `cargo dist`; keep version bumps and dist metadata coordinated when preparing a release.
