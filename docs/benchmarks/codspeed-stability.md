# Tier-B same-revision stability

This study implements #205 after the [build identity](codspeed-build-identity.md)
foundation (#204). Benchmark definitions, fixtures, IDs, and timed regions are
preserved. Measurements use CodSpeed simulation, not local wall-clock timing.

## Repeatable procedure

1. Select a branch containing the foundation and freeze its full commit SHA.
   Dispatch `benchmarks-codspeed.yml` on that branch. Check every run's recorded
   checkout SHA; do not mix PR merge checkouts with branch dispatches.
2. Retain the GitHub job/run URL, attempt number, build-identity summary, and
   CodSpeed run URL. Confirm identical compiler/LLVM, tools, target/profile,
   features, lockfile hash, and simulation settings before pooling observations.
3. Repeat the same dispatch without moving the branch. GitHub's rerun operation
   also preserves the original revision, but retain the attempt number because
   the run URL alone does not distinguish attempts. In this study CodSpeed
   updated the existing result page on rerun; snapshot its results before
   repeating. Prefer separate dispatches from a frozen branch for independent
   retained run pages and independently provisioned hosted environments.
4. Transcribe each case's primary simulated duration from its CodSpeed run page,
   retaining the displayed unit and precision. Retain exportable instruction and
   cache metrics if available. Mark missing metrics unavailable; local timing or
   locally generated cache counts are not substitutes for CodSpeed observations.
5. Measure the complete current suite: all four scalars, all three 72-element
   extraction arrays, dynamic adapt/scalar/array lookup, 5/20/47-field adapters,
   and all three aggregate cases. Separate environment groups when CodSpeed
   identifies differences; an absent warning is not proof of equivalence.
6. Begin with independent repeats to establish the observed range, then add
   another repeat to test whether the range and case classifications persist.
   Continue when new extrema or environment groups change the interpretation.
   Stop with an explicit unresolved classification if evidence does not converge
   or the required diagnostics cannot be retained. There is no predetermined
   sample count or universal acceptable variance threshold.
7. For each case report sample count, minimum, maximum, median, and relative
   range `(maximum - minimum) / median`. This descriptive range is not a
   confidence interval or a merge threshold. Record rounding limits. Recheck
   candidate stable and noisy cases with a further complete-suite run before
   using the classification to justify maintenance changes.

Example dispatch (replace the branch with the frozen study branch):

```text
gh workflow run benchmarks-codspeed.yml --ref codex/203-stability-baseline
gh run list --workflow benchmarks-codspeed.yml --branch codex/203-stability-baseline
```

## Interpretation

Use "observationally stable over these runs" for a case whose measured range
remains small enough to resolve the particular comparison under investigation.
Use "observed variation" when same-build repeats materially change the signal.
Reserve a causal explanation for measured evidence. Allocator, instruction,
cache, and environment effects remain bounded hypotheses without diagnostics.
Use "unresolved" when duration or environment evidence is insufficient.

Any proposed remedy must cite this evidence, preserve the experiment's intent,
and receive a new identity/baseline when its timed semantics change. Small
cases and large array/adapter controls receive the same evidence standard.

## Recorded study

The initial frozen revision is
`6d4a6cec2dbf77f3add8556f20a162d4362e97c8` (foundation PR #207,
preserved on `codex/203-stability-baseline`). Later foundation changes tighten
xtask checks and documentation; they do not change this study's frozen revision.
All five measurements (A1, B, A2, C, D) used the exact recorded identity:
Rust 1.88.0 / LLVM 20.1.5, Cargo 1.88.0, cargo-codspeed 5.0.2,
runner 5.2.1, action `373d6868929f444bc08d901fd0eb0ad52a8875ea`,
Linux x86-64, bench profile, default + benchmark features, simulation mode, and
lockfile SHA-256
`5190700d43d3bd0d49f7e5ac498e1de028204a12c99b10cecbd725bf550ff0d9`.
The [retained dataset](data/codspeed-2026-10-03.json) records every primary value,
run/attempt, build identity, diagnostic availability, and limitation.

| Observation | GitHub provenance | CodSpeed result |
| --- | --- | --- |
| A1 | [37142209362 attempt 1](https://github.com/racedirector/iracing.rs/actions/runs/37142209362/attempts/1) | [Run page](https://app.codspeed.io/racedirector/iracing.rs/runs/6ac141795a7c12fc841dd1b6), now replaced by A2; A1 values were transcribed before rerunning |
| B | [37142235307 attempt 1](https://github.com/racedirector/iracing.rs/actions/runs/37142235307/attempts/1) | [Retained run](https://app.codspeed.io/racedirector/iracing.rs/runs/6ac1418ad7c76c0a90c6bc94) |
| A2 | [37142209362 attempt 2](https://github.com/racedirector/iracing.rs/actions/runs/37142209362/attempts/2) | [Retained rerun](https://app.codspeed.io/racedirector/iracing.rs/runs/6ac141795a7c12fc841dd1b6) |
| C | [37142520863 attempt 1](https://github.com/racedirector/iracing.rs/actions/runs/37142520863/attempts/1) | [Retained run](https://app.codspeed.io/racedirector/iracing.rs/runs/6ac14303f4eddb2aa0ba40bb) |
| D | [37142728248 attempt 1](https://github.com/racedirector/iracing.rs/actions/runs/37142728248/attempts/1) | [Retained run](https://app.codspeed.io/racedirector/iracing.rs/runs/6ac1437484a65ad720526844) |

A1/B established the f64 range; A2 checked its reproducibility and revealed
CodSpeed's overwrite behavior. C independently checked the whole suite and
revealed boolean-scalar variation. D rechecked those cases and controls, and
revealed f32 variation. Five is the resulting exploratory sample count, not a
prescribed statistical minimum. Stop here with **unresolved causal attribution**
and an explicitly bounded observed range: the small sample and unavailable
cache/environment diagnostics do not support a universal variance bound.
Additional repeats remain appropriate for a particular future disputed delta.
Earlier pilot revisions and PR merge checkouts are excluded.

### Quantitative observations

All values below are **nanoseconds**, converted from the displayed ns/us values.
Each row has n=5. The public UI rounds larger values to tenths of a microsecond
and scalar values to tenths of a nanosecond, stripping trailing zeros.
Equal displayed values establish stability only at that display precision.
The relative range is descriptive, not an efficiency percentage or threshold.

| Case | A1 / B / A2 / C / D (ns) | Min | Max | Median | Range / median |
| --- | --- | --- | --- | --- | --- |
| `derived_adapters/large_frame[47_fields]` | 23300 / 23400 / 23400 / 23500 / 23200 | 23200 | 23500 | 23400 | 1.28% |
| `derived_adapters/medium_frame[20_fields]` | 9900 / 9900 / 9900 / 10000 / 9900 | 9900 | 10000 | 9900 | 1.01% |
| `derived_adapters/small_frame[5_fields]` | 2900 / 2900 / 2900 / 2900 / 2800 | 2800 | 2900 | 2900 | 3.45% |
| `dynamic_frame/adapt` | 719.6 / 719.6 / 719.6 / 719.6 / 719.6 | 719.6 | 719.6 | 719.6 | 0.00% |
| `dynamic_frame/array_hit_72` | 11900 / 11900 / 11900 / 11900 / 11900 | 11900 | 11900 | 11900 | 0.00% |
| `dynamic_frame/scalar_hit` | 3000 / 3000 / 3000 / 3000 / 3000 | 3000 | 3000 | 3000 | 0.00% |
| `aggregate_full_frame/representative_consumer/fresh_outputs_47_fields_3_arrays` | 44100 / 44200 / 44200 / 43900 / 44000 | 43900 | 44200 | 44100 | 0.68% |
| `aggregate_full_frame/telemetry_value_all/fresh_outputs` | 315900 / 315300 / 315900 / 316000 / 315600 | 315300 | 316000 | 315900 | 0.22% |
| `aggregate_full_frame/telemetry_value_scalars/fresh_outputs` | 59200 / 59000 / 59100 / 59500 / 59800 | 59000 | 59800 | 59200 | 1.35% |
| `array_extraction/bool_array[72]` | 8700 / 8600 / 8700 / 8700 / 8700 | 8600 | 8700 | 8700 | 1.15% |
| `array_extraction/f32_array[72]` | 9900 / 9700 / 9900 / 9700 / 9800 | 9700 | 9900 | 9800 | 2.04% |
| `array_extraction/i32_array[72]` | 9600 / 9500 / 9500 / 9600 / 9700 | 9500 | 9700 | 9600 | 2.08% |
| `scalar_extraction/bool_driver_marker` | 693.7 / 693.7 / 693.7 / 747.9 / 693.7 | 693.7 | 747.9 | 693.7 | 7.81% |
| `scalar_extraction/f32_speed` | 638.8 / 638.8 / 638.8 / 638.8 / 693 | 638.8 | 693 | 638.8 | 8.48% |
| `scalar_extraction/f64_session_time` | 527.8 / 582 / 527.8 / 527.8 / 527.8 | 527.8 | 582 | 527.8 | 10.27% |
| `scalar_extraction/i32_gear` | 693.9 / 693.9 / 693.9 / 693.9 / 693.9 | 693.9 | 693.9 | 693.9 | 0.00% |

### Findings and bounded explanations

- **Observed scalar variation:** f64 session time spans 527.8–582.0 ns (10.27%
  relative range); boolean marker spans 693.7–747.9 ns (7.81%); f32 speed spans
  638.8–693.0 ns (8.48%). A2/C/D reproduce the lower f64 observation, and D
  returns the boolean case to its prior lower value. These fluctuations happen
  without source/compiler/lockfile changes. A similarly sized future delta in
  these cases needs repeated measurement before attribution.
- **Observationally stable controls in this sample:** dynamic adapt, scalar
  lookup, dynamic array lookup, and i32 extraction have identical displayed
  values. Array extraction ranges are 1.15–2.08%; 20/47-field adapter ranges are
  1.01/1.28%; aggregate ranges are 0.22–1.35%. The 5-field adapter spans one
  display increment (3.45%). This is sample-specific evidence for retaining
  the current experiments, not proof they cannot vary under other identities.
- **Causality unresolved:** the three scalar swings are approximately 54.2 ns,
  a bounded clue consistent with an execution/simulation cost difference.
  The f64/boolean cases do not allocate their decoded output, so blaming their
  timed output allocator is unsupported. Binary layout, cache effects, or other
  runtime/simulation behavior are hypotheses requiring diagnostics; this study
  does not establish a host-derived cache model or one particular missed cache.
- **Diagnostics:** per-case CPU profiles are available. For example the
  [B f64 profile](https://app.codspeed.io/racedirector/iracing.rs/runs/6ac1418ad7c76c0a90c6bc94?uri=crates%2Firacing-sdk%2Fbenches%2Fvar_data_extraction.rs%3A%3Abenches%3A%3Abench_scalar_extraction%3A%3Ascalar_extraction%3A%3Af64_session_time&runnerMode=Simulation)
  shows function-origin proportions of 69.49% for `f64::from_bytes`, 20.32% for
  `black_box<&[u8]>`, and 9.58% for `get<u8>`. These are profile proportions,
  not exported instruction or cache counts. Raw instructions, L1/last-level
  misses, and full runtime compatibility classifications were unavailable in
  the inspected public views and job logs. The exact-run UI search did not
  expose a comparison by run ID. No environment-equivalence claim is made.
- **Maintenance:** investigate only the demonstrated scalar signal; do not
  batch or change dynamic adapt, arrays, or large adapters from this evidence.
  A focused follow-up can obtain diagnostics and evaluate retaining a scalar
  as diagnostic-only or adding a separately identified throughput experiment.
  No workload or timed boundary is changed by this study.
