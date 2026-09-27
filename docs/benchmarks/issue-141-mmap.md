# Issue #141: mmap storage comparison

Base: #140 / PR #152, `5c976e4165617119df837b0e7c8d48db869724b0`.
Head: working changes on `feature/issue-141`. Measured on Windows with
rustc 1.97.1, release profile, on the same machine and fixtures. Benchmark
definitions are unchanged from the base. These are Criterion **quick-mode**
point estimates, not statistically established performance improvements.

| Case | Fixture | File base | Mmap head |
| --- | --- | ---: | ---: |
| Open and drop | 5.9 MB | 15.806 µs | 35.265 µs |
| Open and drop | 142.6 MB | 15.532 µs | 30.954 µs |
| Sequential reader replay | 5.9 MB | 6.9031 ms | 1.8081 ms |
| Sequential reader replay | 142.6 MB | 199.15 ms | 53.834 ms |
| 1,024 indexed random reads | 5.9 MB | 2.1559 ms | 1.0281 ms |
| 1,024 indexed random reads | 142.6 MB | 2.9575 ms | 1.8644 ms |
| Provider replay | 5.9 MB | 7.7965 ms | 2.2804 ms |
| Provider replay | 142.6 MB | 215.59 ms | 58.774 ms |
| Connection replay | 5.9 MB | 22.246 ms | 16.350 ms |
| Connection replay | 142.6 MB | 546.81 ms | 384.10 ms |

The observed tradeoff is increased construction cost and reduced replay/read
cost. Normal OS caching applies; these are not cold-cache measurements.
Criterion quick-mode significance tests did not establish a significant change.

The unchanged `ibt_reader_measurement` diagnostic reported zero retained heap
bytes for both file and mmap sources, with peak open allocations of 350 bytes
(small) and 269 bytes (large) for both. Mapped virtual memory and resident pages
are not application heap and are not counted by this diagnostic. The owned
baseline retained 5,945,258 and 142,594,718 bytes respectively.

Commands, run first at the base and then with the mmap changes:

```text
cargo bench -p iracing-sdk --features benchmark --bench ibt_reader_performance -- --quick --save-baseline issue-141-base
cargo bench -p iracing-sdk --features benchmark --bench ibt_reader_performance -- --quick --baseline issue-141-base
cargo bench -p iracing-sdk --features benchmark --bench ibt_reader_measurement
```

The agreed contract is completed, immutable recordings for the lifetime of the
mapping. Tests that changed a live mapped file now drop the reader first;
provider short-read/retry tests inject failures into owned bytes instead.
The source helper already lives in `reader/source.rs` on #140, so it is part of
this change as well as `reader/mod.rs`. No benchmark targets were changed.

Linux execution remains pending: the local Ubuntu installation has no Rust
toolchain. Existing Windows/Linux CI should validate the branch before merge.
