# Testing and Fixtures

## Deterministic telemetry data

The canonical generated fixtures are:

```text
test-data/
  ibt/
    manifest.json
    profile_small.ibt
    profile_medium.ibt
    profile_large.ibt
  session-yaml/
    profile_small.yaml
    profile_medium.yaml
    profile_large.yaml
```

`scripts/generate_test_fixtures.py` defines profiles and writes both `.ibt` and
session YAML files. The binary layout deliberately follows the SDK's disk/header
constants. `scripts/verify_test_fixtures.py` checks manifest invariants, hashes,
headers, frame geometry, variables, and companion YAML.

`scripts/check_test_fixtures.py` is the normal entry point. It:

1. regenerates fixtures;
2. verifies them;
3. runs `git diff --exit-code` over generated fixture directories.

Use `--no-drift-check` only while intentionally changing generated output.

## Manifest contract

When `test-data/ibt/manifest.json` exists, its listed profiles are authoritative
for `iracing_sdk::test_utils` discovery. Unlisted real-world captures in the same directory
are not automatically part of deterministic manifest-backed tests.

The manifest records layout constants, profile seeds, frame counts and sizes,
header offsets, session metadata, hashes, and required variables. Change the
generator and verifier together when this contract changes.

## Shared helpers

`iracing_sdk::test_utils` finds the repository root by walking upward for a `.git` entry and
resolves `test-data` from there.

Use:

- `require_ibt_fixtures` for tests that need all generated profiles;
- `require_named_ibt_fixture` for a semantic named case;
- `require_smallest_ibt_fixture` for repeated or performance-sensitive tests;
- `load_fixture_manifest` when assertions should be driven by generated
  invariants.

The `require_*` APIs fail with shared regeneration guidance. Do not silently
skip a required integration test because data is absent. The best-effort
`get_*` helpers are for genuinely optional discovery.

## Test layering

- Before designing synthetic telemetry or session inputs, use the generated
  schema catalog in [`docs/reference/README.md`](../reference/README.md) to find
  real variable names and metadata, session shapes, and primitive value
  domains. A disk/live snapshot's offsets and `frame_size` form one captured
  layout and must not be combined with offsets from another snapshot.
- Unit tests should construct minimal frames/schemas and use fake providers or
  ports.
- SDK parser/reader integration tests should use deterministic fixtures through
  `iracing_sdk::test_utils`.
- Session parser unit tests are portable. Manifest-backed provider tests compare
  each embedded document with its required companion under `test-data/session-yaml`;
  only the explicit Windows live smoke test may be ignored for simulator availability.
- Broadcast application tests should fake internal ports; transport tests should
  exercise tonic over a real listener where platform gates permit.
- Simulation tests should inject `SimStatusClient`; only an explicit smoke test
  should touch a live endpoint.
- Examples are compiled in CI and should use public APIs.

## CI-aligned gates

The root quality workflow runs on Ubuntu and Windows:

```text
python scripts/check_test_fixtures.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings
cargo test --workspace --all-targets
```

The docs workflow conditionally runs doctests, rustdoc with warnings denied, and
examples/binaries checks for `iracing-sdk`, `iracing-sdk-derive`, and
`iracing-simulation` on Ubuntu and Windows.

Choose checks by affected boundary, but before pushing broad changes match the
workflow rather than relying on a narrower local test.
