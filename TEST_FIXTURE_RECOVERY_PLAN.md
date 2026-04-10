# Test + Fixture Recovery Plan (Post-Git-LFS)

## Context

The workspace currently assumes telemetry fixtures (`.ibt`) are available in `test-data/ibt` and downloaded via Git LFS. We no longer have access to the original LFS remote, so we need to:

1. make tests runnable without external LFS access,
2. generate and version our own deterministic fixtures, and
3. update tests that currently hard-code values from the old recordings.

## Current failure surface

### 1) Workspace-level blocker

The workspace member at `crates/iracing-sdk-adapter` has already been removed from `Cargo.toml`, and the adapter surface now lives in `iracing-sdk`. If any CI job or external workflow still points at the old path, it must be updated or restored to a real manifest before the rest of the plan can move forward.

Immediate action: verify that non-Windows builds do not expose the live adapter surface, then remove any remaining stale CI inputs or restore the manifest if the compatibility crate truly needs to exist again.

### 2) Fixture dependency hotspots

The strongest fixture coupling is in:

- `crates/iracing-sdk/src/ibt/format.rs` tests:
  - expect **specific fixture filenames**,
  - assert **exact header values** (var counts, offsets, frame counts, etc.),
  - assume exactly 3 fixtures.
- `crates/iracing-sdk/src/ibt/reader.rs` tests:
  - use `require_smallest_ibt_fixture()` and perform generic behavior assertions.
- `crates/test-utils/src/lib.rs`:
  - helper API assumes fixtures exist and mentions `git lfs pull` in errors/guidance.

## Recovery strategy

## Phase 0 — Unblock test execution (same PR)

1. Verify and fix any non-Windows adapter export mismatch so the live surface stays Windows-only.
2. Run baseline checks:
   - `cargo test -p test-utils`
   - `cargo test -p iracing-sdk --lib`
   - full `cargo test` once fixture work is done.

## Phase 1 — Replace LFS-only assumptions with in-repo generated assets

Add a deterministic fixture generation pipeline that creates synthetic but valid IBT files.

### Files to generate and commit

Create and commit these under `test-data/`:

1. `test-data/ibt/*.ibt`
   - At least **3 generated fixtures** to preserve multi-fixture coverage.
   - Keep current names for compatibility, or introduce new stable names and update tests.

2. `test-data/ibt/manifest.json`
   - Fixture metadata and invariants used by tests.
   - Example fields:
     - `name`, `path`, `seed`, `tick_rate`, `num_vars`, `frame_size`, `num_frames`,
     - required variable offsets/types for smoke assertions.

3. `test-data/session-yaml/*.yaml` (optional but recommended)
   - Canonical session YAML payloads used when generating IBT internals.
   - Keeps fixture generation diff-friendly.

4. `scripts/generate_test_fixtures.(rs|py)`
   - The generator is deterministic (seeded).
   - Should be runnable in CI and locally.

5. `scripts/verify_test_fixtures.(rs|py)` (optional but recommended)
   - Verifies that committed fixtures match manifest expectations.

## Phase 2 — Reshape tests around invariants, not historical recordings

### `format.rs` tests

Refactor from exact historical constants to fixture-manifest-driven assertions:

- Keep parser correctness checks (header parse, schema parse, frame extraction).
- Replace brittle asserts like `assert_eq!(header.num_vars, 287)` with:
  - `assert_eq!(header.num_vars, fixture_manifest.expected_num_vars)`.
- Preserve cross-fixture comparisons (e.g., fixture A has more vars than fixture B), but base them on generated fixture profiles.

### `reader.rs` tests

Mostly reusable as-is. Add explicit checks that generated fixture invariants hold:

- valid frame count semantics,
- read/seek/eof behavior,
- session YAML parse.

### `test-utils` updates

- Change `FIXTURE_INSTALL_GUIDANCE` to generated-fixture language.
- Add helper to load/validate manifest (`load_fixture_manifest()`).
- Optionally support auto-generation in dev mode (not in CI by default).

## Phase 3 — CI hardening

1. Add a dedicated CI step:
   - run fixture generator,
   - assert no diff (`git diff --exit-code`) or compare checksums.
2. Add a lightweight fixture validation test target:
   - `cargo test -p test-utils fixture_manifest`.
3. Document contributor workflow in README:
   - how to regenerate fixtures,
   - when regeneration is required,
   - expected deterministic behavior.

## Suggested fixture profiles

Use profiles with distinct characteristics so tests still cover variation:

1. `profile_small.ibt`
   - minimal variable set,
   - small frame count,
   - fast tests.

2. `profile_medium.ibt`
   - full baseline variable set,
   - moderate frame count.

3. `profile_large.ibt`
   - extra variables/longer session YAML,
   - larger frame count for perf and seeking tests.

(If backward compatibility matters, alias these to current filenames.)

## Implementation order (recommended)

1. Confirm the non-Windows adapter export surface is gated correctly.
2. Add fixture manifest format + parser in `test-utils`.
3. Implement generator script + produce 3 fixtures.
4. Refactor `format.rs` tests to manifest-driven assertions.
5. Update guidance strings and README docs.
6. Add CI fixture verification step.
7. Run full test matrix.

## Definition of done

- `cargo test` passes in a clean checkout **without Git LFS**.
- No tests require external/private artifact storage.
- Fixture generation is deterministic and documented.
- CI validates fixture integrity.

## Risks and mitigations

- **Risk:** synthetic IBT files miss edge cases from real telemetry.
  - **Mitigation:** keep one "complex" synthetic profile and add targeted parser fuzz/property tests.

- **Risk:** binary fixtures create noisy diffs.
  - **Mitigation:** commit manifest + deterministic generator; optionally store checksums.

- **Risk:** future schema changes break fixture compatibility.
  - **Mitigation:** version manifest (`schema_version`) and gate tests by version.
