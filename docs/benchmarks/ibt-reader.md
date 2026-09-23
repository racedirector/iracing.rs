# IBT reader storage measurement

Issue #84 replaces complete-file retention in `IbtReader::open` with a private
seekable `File` source. The public `from_bytes` path remains intentionally
memory-backed.

## Reproduce

From the repository root, with at least two real `.ibt` captures of 1 MiB or
larger under `test-data`, run:

```text
cargo bench -p iracing-sdk --features benchmark --bench ibt-reader-memory
```

The diagnostic automatically selects the smallest and largest qualifying
recordings. For each it compares:

- `file`: `IbtReader::open(path)`;
- `memory_baseline`: `fs::read(path)` followed by
  `IbtReader::from_bytes(bytes)`, reproducing the previous complete-file
  ownership model without restoring that implementation to production code.

It reports file/frame geometry, construction time, application heap retained
at the end of construction, peak additional heap during construction, replay
time, and sequential MiB/s. Retained/peak bytes come from an instrumented global
allocator and exclude filesystem/kernel page cache. Timing depends on cache
warmth, storage, build profile, and machine load.

## Recorded comparison

Results below were captured after the file-backed stack on 2026-09-19 using an
Apple Silicon macOS development machine. Run the command above for current
numbers on the review machine.

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
justify restoring recording-sized ownership; buffering and mmap remain separate
evidence-driven follow-ups if a real workload needs them.

The acceptance signal is structural rather than a fixed timing threshold:
file-backed retained heap remains metadata-sized as recordings grow, whereas
the memory baseline grows by at least the complete recording size. Filesystem
cache may still grow with bytes read and is explicitly outside reader-owned
memory.
