# SDK performance suite

The performance targets are split by intent. Criterion benchmarks answer stable,
repeatable performance questions. Diagnostics report allocation, memory, or live
environment observations and must not be interpreted as Criterion regressions.

All public target names use kebab-case. The Rust source filenames remain
snake_case by convention.

## Criterion benchmarks

| Target | Measures | Does not measure |
| --- | --- | --- |
| `var-data-extraction` | Representative captured scalar decoding and fresh 72-element typed arrays | Exhaustive type correctness, bounds errors, or whole-frame cost |
| `adapter-performance` | Dynamic lookup and 5-, 20-, and 47-field typed adapter construction from a prepared packet | Provider, connection, or subscription work |
| `aggregate-frame-parsing` | Fresh owned outputs for all variables, a representative consumer, and all scalars | Frame acquisition or delivery |
| `telemetry-delivery` | Deterministic provider-to-adapted-subscriber delivery, coalescing, and acknowledgement backpressure | IBT I/O, Windows shared memory, or simulator pacing |
| `ibt-reader-performance` | File opening, sequential replay, and deterministic random frame access | Retained heap or cross-machine filesystem comparisons |

Compile the full suite with:

```text
cargo bench -p iracing-sdk --features benchmark --no-run
```

Run one target with:

```text
cargo bench -p iracing-sdk --features benchmark --bench <target>
```

The source-level documentation at the top of each target defines its setup,
timed boundary, throughput unit, and interpretation limits.

### IBT reader performance

`ibt-reader-performance` uses checked-in 5.9 MB and 142.6 MB recordings. Its
case names and timed boundaries are intended to remain stable across reader
implementations so pull requests can compare their base and head revisions.
The reader's indexed random-frame calls replaced the old cursor-only seek API;
results across that API cutover are different experiments.

Construction includes the implementation's normal file-opening and metadata
work. Sequential and random-access reader construction occurs outside the timed
routine. Fixtures are prewarmed independently of the reader before timing. Compare
revisions on the same machine and repeat surprising results.

### Captured-schema decoding

`var-data-extraction`, `adapter-performance`, and `aggregate-frame-parsing` use
the checked-in live variable-schema capture to create deterministic, type-correct
bytes. Fixture generation, schema validation, lookups, and sentinel checks occur
before timing.

The fixture is a realistic layout, not recorded driving data. Exhaustive enum,
bitfield, missing-field, default-value, and bounds behavior is covered by tests
rather than by microbenchmarks.

### Deterministic delivery

`telemetry-delivery` requires neither iRacing nor Windows shared memory. Its
controlled provider copies the fixture into a `FramePacket`, passes it through
the production delivery policy, and adapts it into a shared 47-field consumer.

- `latest-paced` releases a source frame after every subscriber consumes the
  previous latest snapshot.
- `latest-burst-8` offers eight frames before subscribers consume the latest;
  replaced versions are intentional.
- `ondemand-acknowledged` preserves every frame and advances after every active
  subscriber acknowledges its prior frame.
- `ondemand-slow-ack` deterministically withholds the last acknowledgement to
  exercise shared-cursor backpressure.

Runtime construction, validation, subscriptions, and shutdown remain outside
the reported duration.

## Diagnostics

| Target | Reports | Interpretation limit |
| --- | --- | --- |
| `telemetry-diagnostics` | Allocation counts, delivery latency percentiles, and replacement/acknowledgement counts | Allocator and timestamp instrumentation perturb the hot path |
| `ibt-reader-memory` | Retained and peak application heap for file-backed and legacy-equivalent in-memory readers | Excludes kernel/filesystem page cache |
| `live-telemetry-diagnostic` | Live cadence, inter-arrival percentiles, and skipped/coalesced ticks | Requires Windows and an active simulator; results are environment-dependent |

Run a deterministic diagnostic with the same `cargo bench --bench <target>`
form. `live-telemetry-diagnostic` is compile-only in hosted CI and should be run
manually on Windows.

`ibt-reader-memory` asserts that the file-backed reader retains less than half
the recording length while the in-memory baseline retains at least the source
length. It also reports sequential replay throughput, but that timing remains
sensitive to filesystem cache warmth and machine load.

## CI coverage

For Tier-B comparisons, use the repository's
[build identity contract](../../../docs/benchmarks/codspeed-build-identity.md),
[same-revision stability study](../../../docs/benchmarks/codspeed-stability.md),
and [regression investigation runbook](../../../docs/benchmarks/codspeed-investigation.md).
Compiler upgrades start deliberate baseline transitions. Review environment
warnings and measured repeat variation before attributing a percentage to
production code.

| Target | Tier | CI interpretation |
| --- | --- | --- |
| `var-data-extraction` | B | CodSpeed simulation regression evidence |
| `adapter-performance` | B | CodSpeed simulation regression evidence |
| `aggregate-frame-parsing` | B | CodSpeed simulation regression evidence |
| `telemetry-delivery` | C | Same-runner wall-time base/head comparison |
| `ibt-reader-performance` | D | Same-runner warm-cache base/head comparison |
| `telemetry-diagnostics` | E | Diagnostic and invariant checks only |
| `ibt-reader-memory` | E | Diagnostic and invariant checks only |
| `live-telemetry-diagnostic` | E | Manual live only; compile-only in hosted CI |

The quality workflow compiles all eight targets on Ubuntu and Windows.
CodSpeed simulates the three deterministic CPU/in-memory targets for affected
pull requests and main pushes. CodSpeed simulation and Criterion wall-time
results are different measurement domains and must not be compared numerically.

The delivery and IBT workflows compare base and head on the same runner for
relevant pull requests. Timing deltas are informational, with no merge-blocking
percentage threshold. Experiment input changes are warned about separately
from production or build input changes. New benchmark IDs are unpaired until a
base revision contains the same definition.

IBT fixtures are sequentially read through plain file I/O before each timing
case, outside its timed routine. These are warm-cache local measurements, not
cold-open disk latency. The reader uses direct indexed frame access for the
1,024-position random-frame case. The former cursor-only seek operation was
removed with the reader API migration, so a seek-only benchmark is unavailable.
