# Investigating Tier-B CodSpeed deltas

Tier B measures deterministic in-memory CPU work with CodSpeed simulation.
Tier C/D same-runner Criterion comparisons and live telemetry remain separate
experiments. Follow this procedure before accepting or dismissing a surprising
percentage. Source inspection alone cannot dismiss a regression; a headline
percentage alone cannot establish one.

## Evidence chain

1. **Experiment and workload:** compare the case ID, benchmark source, timed
   boundary, support code, captured schema, fixtures, and sentinel checks.
   Record each change. A single-operation cost and batched throughput are
   different experiments even when they call the same decoder.
2. **Measured production path:** trace the calls actually executed inside the
   timed region. Inspect their implementation diff and any relevant generated
   code. An unrelated source diff weakens a semantic explanation but does not
   rule out changes to optimized code or runtime behavior.
3. **Build inputs:** compare the [build identity](codspeed-build-identity.md)
   summaries: compiler/LLVM, Cargo/cargo-codspeed, action/runner, target,
   profile/features, lockfile hash, and checked-out SHA. Check Cargo configuration
   and simulation options too. Intentional upgrades are baseline transitions.
4. **Environment:** retain CodSpeed's warning and the exact environment details
   it shows. The current action has no supported repository-readable output
   for its complete compatibility classification. The summary is machine
   inspectable repository provenance; environment warnings remain reviewer
   evidence. Missing repository data does not prove equivalent environments.
5. **Same-revision stability:** consult the [recorded study](codspeed-stability.md)
   for this case and build identity, including its sample count, precision,
   range, and limitations. Repeat the same revision if the existing study does
   not resolve the current question. Do not invent a global threshold.
6. **Diagnostics:** retain instruction/cache metrics and differential profiles
   or flamegraphs where CodSpeed exposes them. Explicitly mark unavailable
   diagnostics. Equal instruction counts do not establish equal cost, and an
   environment warning does not prove a particular CPU/cache mechanism.
7. **Controlled reproduction:** reproduce base/head with intentionally equivalent
   compiler, dependencies where appropriate, target, profile, features, and
   simulation options. Retain provenance for both. If changing dependencies is
   the proposed regression, preserve that change and isolate other inputs.
   Ordinary hosted-runner wall-clock timing is not historical Tier-B evidence.
8. **Experiment maintenance:** redesign only a demonstrated unsuitable case.
   Record measured instability, current intent/boundary, the smallest proposed
   remedy, and why it addresses the evidence. Assign a new case identity and
   baseline for changed semantics, unless a documented equivalence argument is
   supported by measurements. Preserve stable controls. Batching or allocation
   exclusion are options to evaluate, not default fixes.
9. **Classify and report:** choose one of the states below and link the evidence.
   Leave missing evidence explicit instead of forcing a verdict.

Simulation avoids ordinary wall-clock runner-load timing as the primary model.
It does not make different binaries or runtime behavior equivalent experiments.
Compiler/LLVM, target features, linked libraries, allocator behavior, dependencies,
and environment changes require scrutiny. An environment mismatch weakens a
comparison; it does not automatically make the result noise. Do not reconstruct
CodSpeed-internal compatibility from runner labels or guessed metadata.

## Classification requirements

| State | Required evidence and next action |
| --- | --- |
| Likely production regression | Comparable experiment/build inputs, a reproducible delta exceeding observed same-revision variation for this question, and measured-path/profile evidence. Investigate and fix the production cause. |
| Intentional experiment/baseline transition | An explicit reviewed workload, timed-boundary, or build-input change with old/new identity recorded. Establish a new baseline and stability evidence. |
| Incompatible comparison | Identified relevant input/environment differences prevent interpreting the percentage as equivalent-input evidence. Reproduce under controlled inputs; do not claim the delta is noise. |
| Unresolved/noisy signal | Missing evidence, inconsistent repeats, or observed variation prevents attribution. State whether noise was actually measured or uncertainty remains; collect the missing evidence. |

## Evidence record template

```text
Case IDs and CodSpeed base/head run URLs:
Checked-out SHAs and GitHub run/attempt URLs:
Experiment/timed-boundary and fixture/schema changes:
Measured production path and diff:
Build provenance comparison and intentional transitions:
CodSpeed environment warning/details (or unavailable):
Same-revision study/build identity, repeats, range, precision:
Instruction/cache/profile evidence (or unavailable):
Controlled base/head reproduction:
Classification, bounded hypotheses, missing evidence, next action:
Experiment redesign needed? Evidence and new identity/equivalence rationale:
```

## PR #202 evidence exercise

The original [report](https://github.com/racedirector/iracing.rs/pull/202#issuecomment-5960930418)
compares head `874c7ff` with base `bf5d8f9`: dynamic adapt 505.5 to 615.3 ns
(-17.83% efficiency), and f64 session time 583.4 to 694.5 ns (-16%). These are
efficiency percentages from CodSpeed, not percent increases in duration.

The [investigation](https://github.com/racedirector/iracing.rs/pull/202#issuecomment-5970168036)
reports a runtime-environment warning, unchanged benchmark definitions and
directly measured paths, and separate reproduction at head `8979c38` rather
than the report's `874c7ff`. It reports equal instruction counts (33 for f64,
36 for adapt) and 3–4 versus 3 L1 misses for f64. Those are cited third-party
observations, not this repository's new same-revision dataset; they do not
justify declaring the original comparison a proven non-regression.

Source comparison of `bf5d8f9a6665d864fd33c7d5b7e34c02973f55a6` and
`874c7ffb2c8f84c794422bf030c65d5756ada07a` shows no diff in the benchmark
directory, `types/var_data.rs`, or `types/dynamic_frame.rs`. Current f64 timing
decodes one scalar via `VarData::from_bytes` and the checked little-endian decode
macro; dynamic adapt clones the packet's data/schema Arcs and copies the tick.
Preparation and validation stay
outside both timed loops. Exact historic compiler/build provenance is absent
from those comments, because the workflow used floating stable. The current
study's pinned Rust 1.88.0 build is a distinct identity and cannot retroactively
establish compatibility for the historical comparison.

The #205 study records five measurements of `6d4a6ce` under one verified build
identity. Its f64 case spans 527.8–582.0 ns (10.27% relative range), while dynamic
adapt stays at 719.6 ns at displayed precision. This demonstrates current f64
signal variation but supplies no evidence for redesigning dynamic adapt under
this identity. The f64 function profile is available; raw instruction/cache
counts and full runtime compatibility are unresolved. Those current observations
do not establish either the historic f64 delta's cause or a general 10% threshold.

Historic full environment compatibility, raw cache/profile exports, and
controlled reproduction of the report's exact base/head with recorded build
identity remain unavailable in this evidence record. Classification:
**unresolved attribution with a reported environment incompatibility**.
Next action: use the new stability results to frame follow-up measurements,
then reproduce the exact historical source pair under controlled inputs if
attribution of #202 is still required. Neither blanket batching nor a production
fix is justified by the headline percentage alone.
