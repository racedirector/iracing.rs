# Recorded-file storage and delivery measurements

The public `IbtFile::open` path retains a read-only memory map and establishes
immutable metadata/layout once. `IbtFile::from_bytes` intentionally owns the
complete supplied recording. Historical target/group identifiers remain stable
so CI can compare workload history across the #299 architecture migration.

Current methodology is owned by the [memory diagnostic source](../../crates/iracing-sdk/benches/ibt_reader_measurement.rs);
use the [operator guide](../../crates/iracing-sdk/benches/README.md) for current invocation.

## Reproduce

From the repository root, with at least two real `.ibt` captures of 1 MiB or
larger under `test-data`, run:

```text
cargo bench -p iracing-sdk --features benchmark --bench ibt-reader-memory
```

The diagnostic automatically selects the smallest and largest qualifying
recordings. For each it compares:

- `file`: `IbtFile::open(path)`;
- `memory_baseline`: `fs::read(path)` followed by
  `IbtFile::from_bytes(bytes)`, reproducing the complete-file
  ownership model without restoring that implementation to production code.

It reports file/frame geometry, construction time, application heap retained
at the end of construction, peak additional heap during construction, replay
time, and sequential MiB/s. Retained/peak bytes come from an instrumented global
allocator and exclude filesystem/kernel page cache. Timing depends on cache
warmth, storage, build profile, and machine load.

The `ibt-reader-performance` Criterion target preserves the historical
`ibt_reader_open`, `ibt_reader_sequential`, and `ibt_reader_random` group names
while exercising file construction and direct frame reads. The historical
`ibt_provider_sequential_replay` case uses feature-gated
`benchmarking::RecordedProvider` over the canonical file/replay and packet bridge.
`ibt_connection_sequential_replay` uses `benchmarking::OnDemandPipeline::recorded`
to retain the measured acknowledgement and one-time session fetch/parse before
frames. These labels identify experiment workloads, not additional SDK public
recording APIs. Synthetic delivery experiments remain session-free through the
separate harness mode; timing/setup boundaries and iteration counts are preserved.

Run the retained infrastructure tests explicitly:

```text
cargo test -p iracing-sdk --features benchmark --lib benchmarking::
```

## #299 migration measurement

On 2026-10-11, Windows MSVC / Rust 1.97.1 ran the unchanged Criterion workloads
against a `pre-299` baseline saved from `f7956e7`, then the #305 retirement tree.
The same Donington (5,945,258 B) and Bristol (142,594,718 B) captures, prewarming,
runtime, sampling, and timed/setup boundaries were retained. These are local
wall-time measurements, not CodSpeed simulation results. Values below are the
sample means from Criterion's `estimates.json` files.

| Workload | Small before / after | Change | Large before / after | Change |
| --- | ---: | ---: | ---: | ---: |
| Mapped open | 38.402 / 140.798 µs | +266.6% | 45.010 / 145.524 µs | +223.3% |
| Direct sequential frames | 1.791 / 1.825 ms | +1.9% | 48.643 / 47.890 ms | -1.5% |
| Async packet workload | 2.176 / 2.071 ms | -4.8% | 59.204 / 52.148 ms | -11.9% |
| Acknowledged workload | 15.553 / 14.862 ms | -4.4% | 361.800 / 350.167 ms | -3.2% |
| 1,024 random frames | 0.998 / 1.037 ms | +3.9% | 1.962 / 1.968 ms | +0.3% |

The old open workload constructed only a map, headers, and physical geometry.
The canonical open additionally snapshots variable/session metadata and validates
one semantic layout. A separate attribution diagnostic compared those exact
obligations: old partial open, old open plus both snapshots and one
`Arc<TelemetryLayout>`, and canonical open. With the same compiler and warm
captures, fourteen alternating rounds of 1,000 constructions per mode included
drop and blackboxed the returned state; no frames were read or YAML parsed.
Median small/large times were 41.7/40.7 µs for partial open, 143.8/145.5 µs for
complete old setup, and 132.0/136.5 µs for canonical open. This diagnostic accounts
for the added one-time metadata work; it is not a statistical speedup claim.

Small random access showed a statistically detected increase in this run;
large random access and small sequential traversal showed no significant change.
Native frames now retain shared bytes and exact layout identity. Their Arc
reference-count work is a plausible source of the approximately 38 ns additional
cost per small random record, but that cause was not isolated. There is still
one frame allocation/copy, O(1) addressing, and no intermediate Vec; packet
conversion transfers those allocations instead of copying again.

The allocator diagnostic retained 127,164/157,645 B for small/large mapped files,
with the same peak additional construction heap. Owned-file modes retained
6,072,422/142,752,363 B. A recording 24 times larger therefore adds only 30,481 B
of mapped-file metadata ownership; neither mapped open nor lazy traversal copies
the complete recording.

## Historical recorded comparison

Results below were captured after the historical seekable-file implementation
on 2026-09-19 using an Apple Silicon macOS development machine. They precede the
current mmap-backed file model and remain dated comparison evidence. Run the
command above for current numbers on the review machine. An exact measured
revision and compiler version were not retained for that historical experiment.

| Mode | File size | Frame size | Frames | Open | Retained heap | Peak open heap | Replay | Throughput |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| File | 5,945,258 B | 1,107 B | 5,306 | 1.823 ms | 109,770 B | 121,054 B | 6.663 ms | 840.7 MiB/s |
| Memory baseline | 5,945,258 B | 1,107 B | 5,306 | 1.211 ms | 6,054,894 B | 6,066,178 B | 0.560 ms | 10,001.4 MiB/s |
| File | 142,594,718 B | 1,070 B | 133,169 | 0.633 ms | 143,240 B | 143,240 B | 76.826 ms | 1,768.8 MiB/s |
| Memory baseline | 142,594,718 B | 1,070 B | 133,169 | 20.357 ms | 142,737,851 B | 142,737,851 B | 6.687 ms | 20,321.1 MiB/s |

These are single diagnostic passes, not statistically sampled Criterion
estimates. The large recording is about 24 times the size of the small one; the
file-backed reader retained only 33,470 additional heap bytes, while the memory
baseline retained 136,682,957 additional bytes. Sequential file replay remained
above 840 MiB/s in both runs. The expected in-memory replay advantage does not
justify restoring recording-sized ownership. Current `open` uses mmap; this
historical table does not measure that implementation.

The acceptance signal used for that experiment was structural rather than a fixed timing threshold:
file-backed retained heap remains metadata-sized as recordings grow, whereas
the memory baseline grows by at least the complete recording size. Filesystem
cache may still grow with bytes read and is explicitly outside reader-owned
memory. Current metadata snapshots and semantic layout construction are included
in the diagnostic's construction boundary; no frame region is eagerly copied.
