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

Construction includes the implementation's normal file-opening and metadata
work. Sequential and random-access reader construction occurs outside the timed
routine. Filesystem cache state is not controlled, so compare revisions on the
same machine and repeat surprising results.

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

The quality workflow compiles every benchmark and diagnostic target on Ubuntu
and Windows. The benchmark workflow quick-runs all deterministic Criterion
targets for relevant pull requests and pushes, then runs the two deterministic
diagnostics separately.

Weekly and manually requested full runs use normal Criterion sampling. Pull
requests also compare `ibt-reader-performance` at the base and head revisions on
the same runner. The comparison is informational; changed definitions, fixtures,
generation code, or Cargo inputs reduce comparability and are reported in the
workflow summary.

Use Criterion results only for like-for-like case names, fixture revisions,
build profiles, machines, and allocation policies. Performance reports are not
correctness thresholds.
