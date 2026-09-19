# IBT reader performance baseline

This report establishes the timing baseline for the IBT storage refactor in
issue #84. The benchmark target is intentionally checked in before the implementation
changes so the same cases and timed boundaries can be run on both revisions.

## Reproduce

Run only the focused reader target from the repository root:

```text
cargo bench -p iracing-sdk --features benchmark --bench ibt_reader_performance
```

Do not substitute the complete benchmark suite when comparing the reader
implementation. Criterion data is written under `target/criterion` and can be
retained between revisions for its automated change analysis.

## Main baseline

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

The baseline reader loads the complete recording during construction. Its
subsequent replay and seek cases operate on retained memory, so the storage
refactor is expected to improve construction time and retained heap while
making replay and random seek perform actual file IO. Compare those tradeoffs;
do not treat every higher timing as a regression independently of the memory
objective.

Construction and seek setup for the replay/seek groups is outside those timed
routines. Replay includes allocation and copying of each returned owned frame.
The seek group does not read a frame after positioning. Retained heap and page
cache behavior are outside this timing target and require the separate #91
measurement.
