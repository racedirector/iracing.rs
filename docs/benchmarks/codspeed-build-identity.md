# Tier-B build identity

Use the [deterministic workflow](../../.github/workflows/benchmarks-codspeed.yml)
for the exact benchmark compiler, tools, runner, target, profile, features,
simulation options, execution matrix, and emitted identity fields. These are
benchmark build inputs; support/MSRV policy is a separate decision. The
[workflow checks](../../xtask/src/workflows.rs) and their tests own enforcement.

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
