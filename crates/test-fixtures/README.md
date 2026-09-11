# test-fixtures

Workspace-only tooling for generating and verifying the deterministic `.ibt`
fixtures used by `iracing-sdk` tests. This crate replaces the former Python
scripts and deliberately depends on `iracing-sdk` as the source of truth for
iRacing wire structures.

The crate is not published or included in release artifacts.

## Commands

Run commands from the workspace root through the Cargo alias:

```bash
# Generate, verify, and fail if generated files differ from Git
cargo test-fixtures

# Focused workflows
cargo test-fixtures generate
cargo test-fixtures verify
cargo test-fixtures check

# Regenerate an intentional fixture change without the final Git drift failure
cargo test-fixtures check --no-drift-check
```

`verify` is read-only. `generate` and `check` rewrite the canonical files under
`test-data/ibt` and `test-data/session-yaml`; do not run them over fixture edits
that have not been preserved elsewhere.

The library exposes the same workflows as `generate(repo_root)`,
`verify(repo_root)`, and `check(repo_root, drift_check)` for isolated tests and
other workspace tooling.

## IBT layout contract

Generated recordings use this exact order:

```text
0..112                                      irsdk Header
112..144                                    IBT DiskSubHeader
144..(144 + variable_count * 144)           VariableHeader array
session_info_offset..session_info_end        session YAML bytes
session_info_end..EOF                        fixed-size telemetry frames
```

Important distinctions:

- `Header::WIRE_SIZE` is always **112 bytes**.
- `DiskSubHeader::WIRE_SIZE` is **32 bytes**.
- The complete IBT preamble is therefore **144 bytes**.
- `VariableHeader::WIRE_SIZE` is **144 bytes**, and the first variable header
  begins at offset 144—not 112.
- The disk sub-header begins at offset 112 and must satisfy
  `disk_sub_header_offset == var_header_offset - disk_sub_header_size`.

Do not introduce a single ambiguous “header size” constant. The main SDK
header and the composite IBT preamble are different concepts.

The manifest remains schema version 1. Its legacy `live_header_prefix_size`
field means the 112-byte main SDK header size in this fixture contract. The
name is retained for compatibility even though this crate generates recorded
IBT data only; live shared-memory behavior is out of scope.

## Determinism

Profiles and frame formulas live in `src/model.rs` and `src/generate.rs`.
`SteeringWheelAngle` uses a profile-seeded `rand_chacha::ChaCha8Rng`. Choosing
the algorithm explicitly avoids relying on `StdRng`, whose implementation is
not a stable reproducibility contract.

Any change to a profile, frame formula, RNG algorithm/version, field order, YAML
text, or binary layout may change fixture hashes. Intentional changes require:

1. Update the implementation and relevant tests.
2. Run `cargo test-fixtures check --no-drift-check`.
3. Review `.ibt`, YAML, and manifest changes together.
4. Run the crate and SDK fixture tests before committing.

The golden hash test is a determinism guard, not a reason to preserve incorrect
data. Update its expected hashes only after confirming that the new bytes are
intentional and structurally valid.

## Generator and verifier boundaries

The generator uses `iracing-sdk` constructors and `WireType::write_to` for
`Header`, `DiskSubHeader`, and `VariableHeader`. Telemetry scalar values are
written explicitly in little-endian form. The complete artifact set is built
in memory before filesystem writes begin.

The verifier intentionally checks the files through multiple independent
views:

- manifest schema and layout invariants;
- SHA-256 and exact file length;
- typed SDK header and variable-header decoding;
- companion YAML equality;
- schema and complete frame iteration through `IbtReader`.

Manifest paths must be repository-relative and cannot contain parent-directory
components. This prevents a malformed manifest from escaping the supplied
repository root.

`WireType` encoding follows the SDK's little-endian/native-layout contract.
All currently supported workspace and CI targets are little-endian. Do not
claim big-endian support without adding explicit byte-order encoding.

## Profiles

The canonical profiles intentionally increase in size and schema breadth:

| Profile | Frames | Frame size | Variables |
| --- | ---: | ---: | ---: |
| `profile_small` | 12 | 48 bytes | 8 |
| `profile_medium` | 24 | 64 bytes | 10 |
| `profile_large` | 48 | 96 bytes | 13 |

Variable names and metadata should remain grounded in the repository's
generated schema catalog under `docs/reference`. Keep each profile's offsets,
types, counts, and frame size internally coherent.

## Development

```bash
cargo test -p test-fixtures --all-targets
cargo clippy -p test-fixtures --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p test-fixtures --no-deps
cargo test -p iracing-sdk --lib ibt::format::tests
cargo test-fixtures verify
```

The default `cargo test-fixtures` ends with a scoped `git diff --exit-code`
over the two generated directories. It is expected to fail while an intentional
fixture change is present but not yet committed.

## File map

- `src/lib.rs` — library API and generate/verify/check orchestration.
- `src/main.rs` — Cargo-facing CLI.
- `src/model.rs` — fixture profiles and manifest schema.
- `src/generate.rs` — YAML, headers, frames, hashes, and artifact writes.
- `src/verify.rs` — structural, manifest, YAML, and `IbtReader` validation.
- `tests/cli.rs` — end-to-end command coverage against an isolated root.

