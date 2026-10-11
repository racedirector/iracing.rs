# Historical IBT reader performance snapshot

This document preserves the pre-refactor snapshot from issue #84. Current
methodology belongs to the [benchmark source](../../crates/iracing-sdk/benches/ibt_reader_performance.rs)
and [storage-neutral workload helper](../../crates/iracing-sdk/benches/support/ibt.rs).
Current performance evidence comes from revision-specific same-runner comparisons.

## Reproduce

Run only the focused reader target from the repository root:

```text
cargo bench -p iracing-sdk --features benchmark --bench ibt-reader-performance
```

Do not substitute the complete benchmark suite when comparing the reader
implementation. Criterion data is written under `target/criterion` and can be
retained between revisions for its automated change analysis.

## Historical pre-refactor snapshot — 2026-09-19

These values are preserved as historical evidence for the storage refactor.
They are not the repository's current performance baseline. Current PR
performance evidence comes from same-runner base/head workflow comparisons.

Captured on 2026-09-19 from `main` commit `8bb0f94`, before the seekable-source
implementation, using Rust 1.93.1 on arm64 macOS 26.5.1. Filesystem cache state
was not reset, so these are warm-cache local results.

| Case | 5.9 MB recording | 142.6 MB recording |
| --- | ---: | ---: |
| Open and construct | 232.90 us | 19.005 ms |
| Replay every frame | 195.45 us | 4.4649 ms |
| 1,024 random seeks | 13.977 us | 13.870 us |

Values are Criterion point estimates. The complete confidence intervals were:

- Open, 5.9 MB: 228.14–240.16 us.
- Open, 142.6 MB: 18.860–19.105 ms.
- Sequential replay, 5.9 MB: 187.71–202.52 us.
- Sequential replay, 142.6 MB: 4.3738–4.5513 ms.
- Random seek batch, 5.9 MB: 13.762–14.262 us.
- Random seek batch, 142.6 MB: 13.594–14.310 us.

## Interpretation

The historical baseline reader loaded the complete recording during construction. Its
subsequent replay and seek cases operated on retained memory. The storage
refactor changed that tradeoff; these point estimates should not be presented
as current regression evidence.

In that historical experiment, construction and seek setup for replay/seek
groups was outside the timed routines. Replay allocated and copied each owned
frame; the seek case did not read a frame after positioning. Retained heap and
page cache behavior were outside that timing target.

## Current methodology

Follow the benchmark and workload source linked above. Cargo owns target names,
and the same-runner workflow owns base/head execution and report publication.
