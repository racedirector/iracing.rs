# AGENTS.md

Guidance for agents working in `crates/test-fixtures`. Read this file and the
crate README before changing fixture profiles, binary layout, manifest fields,
or commands.

## Critical Commands

- `cargo test -p test-fixtures --all-targets` runs generation, corruption, hash,
  and CLI tests.
- `cargo clippy -p test-fixtures --all-targets -- -D warnings` is the crate lint
  gate.
- `RUSTDOCFLAGS="-D warnings" cargo doc -p test-fixtures --no-deps` validates
  maintenance documentation.
- `cargo test -p iracing-sdk --lib ibt::tests` checks the generated data
  through SDK fixture tests.
- `cargo test-fixtures verify` is read-only; use it when existing artifacts must
  not be rewritten.
- `cargo test-fixtures` regenerates, verifies, and checks Git drift. Use
  `--no-drift-check` only for an intentional output update.

## Non-Negotiable Layout

- Preserve the canonical assembly order owned by [the generator](src/generate.rs):
  main header, disk sub-header, variable-header array, session YAML, then frames.
- Derive sizes from `size_of` on the [iracing-irsdk wire types](../iracing-irsdk/src/lib.rs).
  Their ABI assertions/tests own exact geometry; do not redefine independent
  numeric constants or equate the main header with the composite IBT preamble.

The manifest's `live_header_prefix_size` name is legacy. For schema version 1 it
must remain present and equal 112, as enforced by [the verifier](src/verify.rs), even though this crate does not generate live
shared-memory fixtures.

## Ownership Boundaries

- `iracing-irsdk` owns raw wire layouts, constructors, intrinsic decoding, and
  ABI tests. `iracing-sdk` owns consuming parser and runtime validation.
- This crate owns profiles, deterministic values, session YAML, IBT assembly,
  manifest serialization, hashing, verification orchestration, and drift checks.
- Use `IbtFile` direct traversal and `IbtReplay` in verification so every frame is exercised as downstream consumers read it. Preserve independent wire/hash/YAML checks and compare traversed bytes/layouts against that checked source; do not replace verification with only a second hand-written parser.
- Live telemetry, shared memory, real capture rewriting, and schema discovery
  are out of scope.
- Keep the crate `publish = false` and `dist = false`.

## Determinism Rules

- Keep `ChaCha8Rng` explicit and seed it from each profile. Do not switch to
  `StdRng` for fixture bytes.
- RNG changes are allowed, but they are format-data migrations: regenerate,
  inspect, update golden hashes, and run SDK fixture tests.
- Stable hashes cover the complete `.ibt` bytes. Manifest formatting uses
  `serde_json::to_vec_pretty`, struct field order, and one final newline.
- Construct all artifacts successfully before writing any of them.
- Treat generated `.ibt`, YAML, and manifest changes as one atomic review unit.

## Profile Changes

- Consult `docs/reference/README.md` and its disk/session schemas before adding
  telemetry names or shapes.
- Ensure `offset + type_width * count <= frame_size` for every variable.
- Keep profile layouts coherent; do not combine offsets from unrelated captures.
- Update `build_frame` whenever a new declared field requires data. Avoid hidden
  frame writes that are not understood by the profile definition.
- Preserve the small/medium/large progression unless a test requirement justifies
  changing it.

## Verification and Safety

- Verification must remain read-only.
- Generation and check rewrite canonical fixture files. Preserve experimental or
  hand-edited data before running either command.
- Keep manifest paths relative and reject absolute paths or `..` traversal.
- Errors must name the affected path and failed invariant so CI failures are
  actionable.
- The final drift check is deliberately scoped to `test-data/ibt` and
  `test-data/session-yaml`.
- Encoding assumes little-endian targets because `IntoBytes::as_bytes` uses native object
  representation. Add explicit endian conversion before supporting big-endian
  systems.

## Required Validation for Fixture Changes

1. `cargo fmt --all -- --check`
2. `cargo test -p test-fixtures --all-targets`
3. `cargo clippy -p test-fixtures --all-targets -- -D warnings`
4. `RUSTDOCFLAGS="-D warnings" cargo doc -p test-fixtures --no-deps`
5. `cargo test -p iracing-sdk --lib ibt::tests`
6. `cargo test-fixtures verify`

For intentional generated-byte changes, also run
`cargo test-fixtures check --no-drift-check`, inspect the resulting diff and
hashes, and then run the full workspace quality gate before handoff.
