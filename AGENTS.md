# AGENTS.md

Compact guidance for future OpenCode sessions working in this repo.

## Critical Commands

- `cargo build --workspace` for workspace sanity; release builds defer to cargo-dist.
- `cargo test --workspace --all-targets` hits every crate; scope with `cargo test -p <crate>` or `cargo test -p iracing-sdk -- types::tests::bitfield_constructor_works` when debugging.
- `cargo test -p iracing-sdk --doc` and `RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps` replicate the docs CI job.
- `cargo check -p iracing-sdk --examples --bins` mirrors the extra CI check step.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` are the formatting/lint gates.
- Codegen binaries require `cargo build -p iracing-sdk --features codegen,schema-discovery` when you need schema outputs.

## Test Data

- Integration tests expect `.ibt` fixtures from Git LFS; run `git lfs install` once per machine and `git lfs pull` before running suites that touch `test-data/ibt/`.
- Use helpers from `crates/test-utils` (`require_ibt_fixtures`, `require_named_ibt_fixture`, `require_smallest_ibt_fixture`) instead of hardcoded paths so missing-fixture failures stay consistent.

## Workspace Map

- `crates/iracing-sdk`: low-level `.ibt` reader, session YAML parser, streaming adapter APIs, and Windows shared-memory access. All live-telemetry additions must stay behind `#[cfg(windows)]` and keep binaries' `package.metadata.dist.bin.*.targets` in sync.
- `crates/iracing-simulation`: minimal HTTP probe for the sim; no Windows gating.
- `crates/test-utils`: shared fixture plumbing and path discovery; lean on it for integration tests instead of reinventing file lookups.

## Patterns & Gotchas

- Frame extraction is little-endian; always rely on `VarData::from_bytes` rather than manual decoding to avoid drift.
- Session YAML parsing is cached via `SessionInfoParser`; reuse it when adding code so you don't reparse on every frame.
- Live telemetry only compiles on Windows. Non-Windows builds will skip those modules, so gate new APIs and tests accordingly to preserve cross-platform builds.

## CI & Release

- `docs.yml` runs doctests, docs (warnings as errors), and `cargo check` for examples/bins on every PR touching `crates/iracing-sdk/**`; run those targets locally before opening PRs.
- Releases are tag-driven (`v*`) via `cargo dist`; keep version bumps and dist metadata coordinated when preparing a release.
