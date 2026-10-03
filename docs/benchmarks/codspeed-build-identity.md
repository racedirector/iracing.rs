# Tier-B build identity

The deterministic workflow intentionally uses Rust **1.88.0** for benchmark
code generation. This is a benchmark baseline choice, separate from the Rust
1.88 minimum supported compiler contract. Updating the support range does not
automatically update this compiler, and updating this compiler does not change
the support range.

The workflow preserves the three Tier-B targets and their timed boundaries:
`var-data-extraction`, `adapter-performance`, and `aggregate-frame-parsing`.
Its job summary and logs record `rustc -Vv` (including LLVM), Cargo, cargo-codspeed,
the checked-out commit SHA, SHA-256 of `Cargo.lock`, target, profile, features,
simulation mode, and the pinned CodSpeed action and runner. The runner is
explicitly configured using the pinned action's `runner-version` input.

The summary describes repository-owned build inputs. It does not establish
CodSpeed runtime-environment compatibility. Retain the job URL and run attempt
alongside the CodSpeed result URL; download the summary when retaining a study
beyond GitHub's run retention period. Pull-request runs may check out a merge
commit: use the recorded `git rev-parse HEAD`, not an assumed PR head SHA.

## Intentional baseline transitions

Compiler, action, runner, cargo-codspeed, dependency, target, profile, feature,
or simulation-option changes require explicit review as build-input changes.
Record old and new provenance and the reason for the upgrade. Measure the same
source revision under both identities when isolating the build's effect. The
first result under the new identity starts a baseline transition; do not treat
its percentage against the old identity as equivalent-input regression evidence.
Re-establish same-revision stability before interpreting subsequent deltas.

The existing `cargo xtask check-repo` workflow checks reject a floating compiler
and missing identity fields. A deliberate exact-version update remains possible
and visible in the workflow diff. Local Criterion commands remain unchanged.
