# Live Reader Integration Plan

Status: Proposed as of 2026-08-30

## Problem Statement

Maintainers and live-telemetry consumers cannot currently rely on one checked,
internally consistent acquisition boundary for Windows shared memory. Byte-range
validation, header parsing, rotating-buffer selection, retry state, and packet
metadata are split between `windows::Connection` and `LiveProvider`, so malformed
or changing memory can be hidden as `None`, and frame bytes can be labelled using
metadata observed after the frame copy.

## Proposal

Finish the source-neutral reader migration by implementing live acquisition policy
in `reader/live.rs`, adapting the Windows mapping to that reader in
`windows/connection.rs`, and making `providers/live/` perform only provider-level
conversion, waiting, YAML decoding, and packet construction. The work will mirror
the existing `reader::ibt::IbtRecording`/`IbtReader` and `providers::ibt::IbtProvider`
ownership boundary without imposing immutable-recording or cursor semantics on live
telemetry.

There is no PRD or `FEATURE_STATE` registry in this repository as of 2026-08-30.
This plan is the implementation record and depends on
[`telemetry-pipeline.md`](../architecture/telemetry-pipeline.md),
[`platform-and-features.md`](../architecture/platform-and-features.md), and
[`testing-and-fixtures.md`](../architecture/testing-and-fixtures.md).

## Scope

In scope:

- Confirm the live SDK wire ABI before using fields currently modelled in padding.
- Add a portable, deterministic live reader over `RandomAccessSource`.
- Return owned frame bytes together with the tick and session version observed by
  the same accepted acquisition attempt.
- Route Windows connection header, schema, session, and frame reads through the
  reader abstractions.
- Remove the second live-header/buffer lookup from `LiveProvider`.
- Add macOS-runnable unit, property, and Criterion coverage plus Windows runtime
  validation.
- Update public examples, rustdoc, and architecture documentation for changed error
  and snapshot semantics.

Out of scope:

- Changing latest-wins live delivery, update-rate throttling, or connection
  subscription APIs.
- Making session updates lossless; shared memory exposes only the current YAML
  region.
- Changing IBT replay cursor or acknowledgement behavior.
- Replacing `MapViewOfFile`, the Windows event wait, or Tokio scheduling.
- Solving setup-time Win32 handle leaks except where ownership must change to keep
  the mapped view valid during a reader call.
- Adding a compatibility wrapper for the old split-read API. Repository guidance
  requires structural dependency failures to be fixed at their callers.

## Feasibility Assessment and Entry Gates

The portable approach is feasible on macOS because `LiveReader` can be generic over
`RandomAccessSource`; a scripted in-memory source can reproduce writer advancement
between reads without Win32 or iRacing. The current baseline passes all 14
`reader::*` unit tests and `cargo check -p iracing-sdk --all-targets` on macOS. The
existing Windows-only live code is also visible to a macOS-hosted MSVC-target
`cargo check`, which currently exposes pre-existing compile errors in
`providers/live/mod.rs`.

Implementation must not begin past Phase 0 until the following wire-format question
is resolved:

- The repository currently names bytes 40..48 of `Header` as a cached current tick
  and buffer index, and bytes 8..12 of `VariableBuffer` as a begin tick.
- The published SDK definitions describe both locations as padding, and the
  published client scans all advertised buffers for the greatest `tickCount`, then
  rereads that same descriptor after `memcpy`.
- On Windows, compare the installed/current iRacing SDK `irsdk_defines.h` and
  `irsdk_client.cpp` with a captured 112-byte live header. Record the SDK version,
  field offsets, sizes, and observed padding values in a test fixture or audit note.
- If the installed SDK still defines padding, rename those Rust fields back to
  padding and implement max-tick selection. If it defines newer synchronization
  fields, record the SDK version contract and implement that documented protocol.
  Do not infer synchronization semantics from nonzero bytes alone.

Reference material when rust-analyzer cannot establish OS or producer behavior:

- The [published iRacing SDK definitions](https://github.com/vipoo/irsdk/blob/master/irsdk_defines.h)
  for wire layout.
- The [published iRacing SDK client copy loop](https://github.com/vipoo/irsdk/blob/master/irsdk_utils.cpp)
  for newest-buffer selection, reset behavior, and the two-copy-attempt protocol.
- Microsoft documentation for
  [`MapViewOfFile`](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-mapviewoffile),
  [`VirtualQuery`](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualquery),
  and
  [`WaitForSingleObject`](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject)
  for view extent, lifetime, failure, and wait semantics.
- The checked-in [`live-variable-schema.yml`](../reference/live-variable-schema.yml)
  for one coherent 8,586-byte live frame layout used by tests and benchmarks.

## Decisions

| Decision | Choice |
| --- | --- |
| Shared abstraction | Keep `RandomAccessSource` as the shared capability; do not add a common high-level IBT/live reader trait. |
| Live state owner | Add a small `LiveReader` policy object that owns last-seen tick/retry state and borrows a source for each operation. This avoids a self-referential `Connection` while allowing an ephemeral `MappedView<'_>`. |
| Snapshot ownership | Return owned `FrameBuffer`, `SessionInfoBuffer`, and variable-header buffers. Never return a slice or reference into mutable mapped memory. |
| Frame result | Return `Result<Option<LiveFrameSnapshot>>`, where the snapshot contains frame bytes, source tick, and session-info version from one accepted attempt. `None` means connected but no new coherent frame; malformed layout and copy failures remain errors. |
| First observation/reset | Match the ABI-confirmed SDK client: establish/reset the tick baseline without publishing stale data, then publish only a later tick. Lock this behavior with characterization tests before provider changes. |
| Buffer selection | Use the protocol confirmed in Phase 0. The current published protocol scans `buffers[..buffer_count]` for the greatest tick; it does not read padding as a current-buffer cache. |
| Consistency check | Copy from offsets interpreted from one validated header snapshot, then reread the selected descriptor/header fields needed to prove the tick and packet metadata did not change. Retry at most twice, matching the SDK client, and return `None` after contention exhaustion. |
| Session snapshots | Copy the advertised YAML region and accept it only when version, offset, and length are stable across the copy. Return the actual observed version with the owned buffer; do not claim historical lookup by version. |
| Provider boundary | `LiveProvider` converts signed wire metadata, constructs `FramePacket`, and decodes `IRacingSessionString`; it does not reread the mapping to discover tick or session version. |
| Unsafe boundary | `Connection` creates a lifetime-bound `MappedView` from its mapped base/extent for each reader call. All offset arithmetic and copies then go through `RandomAccessSource`. |
| API migration | Change fallible connection reads to return `Result`; update all repository callers in the same phase rather than masking failures as `None` or adding compatibility shims. |
| Performance policy | Benchmark deterministic acquisition separately from event waiting and end-to-end delivery. Do not make host-specific Criterion times a CI pass/fail threshold. |

## Component Architecture

```text
Win32 ownership and pacing
  windows::Connection
    - mapping/event handles and mapped extent
    - wait_for_update[_async]
    - ephemeral MappedView<'_>
             |
             v
Portable acquisition and consistency
  reader::live::LiveReader
    - validated Header snapshots
    - newest-buffer selection
    - last-tick/reset state
    - before/copy/after retry protocol
    - owned LiveFrameSnapshot / LiveSessionSnapshot
             |
             v
Provider interpretation
  providers::live::LiveProvider
    - wait/poll/disconnect policy
    - VariableSchema construction
    - IRacingSessionString conversion
    - FramePacket construction
             |
             v
Existing Telemetry + LatestDelivery + LiveSessionPolicy
```

| Layer | Responsibility |
| --- | --- |
| `reader/access_source.rs` | Checked absolute reads and owned byte copies; no coherence promise. |
| `reader/header.rs` | Convert one header observation into checked metadata/frame regions. |
| `reader/live.rs` | Live-only validation, selection, retry, reset, and coherent owned snapshots. |
| `windows/connection.rs` | Own Win32 resources, expose the mapped source, wait for events, and delegate acquisition. |
| `providers/live/` | Apply provider lifecycle and convert accepted reader snapshots into public packets/YAML. |

Planned file structure:

```text
crates/iracing-sdk/
  src/
    reader/
      live.rs                                      MODIFY
      header.rs                                    MODIFY if stable-field rereads need helpers
      access_source.rs                             UPDATE rustdoc only if contract clarification is needed
    types/irsdk/
      header.rs                                    MODIFY after ABI gate
      variable_buffer.rs                           MODIFY after ABI gate
    windows/
      connection.rs                                MODIFY
    providers/live/
      mod.rs                                       MODIFY
      builder.rs                                   UPDATE only for changed constructor errors
    types/schema.rs                                MODIFY to construct live schema from reader snapshots
    bin/                                           UPDATE live callers affected by fallible reads
    examples/                                      UPDATE live callers affected by snapshot API
    benches/
      live_reader_acquisition.rs                   NEW
      live_frame_latency.rs                        MODIFY
      README.md                                    UPDATE
    Cargo.toml                                     MODIFY for the new benchmark target
docs/
  architecture/telemetry-pipeline.md               MODIFY
  architecture/platform-and-features.md            UPDATE if cfg surface changes
  plans/live-reader-integration.md                  UPDATE phase status during delivery
```

## Data Access Patterns

### Reads

| Data | Preferred approach | Fallback/failure behavior |
| --- | --- | --- |
| Mapped extent | `VirtualQuery` immediately after mapping, retained by `Connection` | Fail connection construction; never guess a mapping size. |
| Header | Copy exactly `Header::WIRE_SIZE` through `RandomAccessSource`, decode with `WireType`, then `validate_live` and validate every advertised region | Return structured parse/memory error; no panic. |
| Variable headers | `HeaderSnapshotReader` using the same validated header observation | `Ok(None)` only for advertised count zero; invalid range is `Err`. |
| Telemetry frame | Select a descriptor from a validated header, copy its complete frame region, then verify required fields after the copy | Retry once; return `Ok(None)` after two contention failures. |
| Session YAML | Copy current session region and verify version/offset/length are stable | Retry according to explicit reader policy; never label mutable bytes as a requested historical version. |
| Connection status/tick rate | Read through a checked header/scalar snapshot helper | Propagate an error or use the last immutable initialization value where explicitly documented. |

### Mutations

No mapped-memory mutation is permitted. Internal mutations are limited to
`LiveReader`'s last-seen tick and diagnostic counters after an accepted observation
or confirmed producer reset.

| State | Mutation rule |
| --- | --- |
| `last_tick_count` | Advance only after an accepted frame; reset according to the confirmed SDK disconnect/tick-regression behavior. |
| Retry counters | Increment on detected concurrent advancement; expose through tracing/benchmark diagnostics, not public mutable state unless needed. |
| Provider no-connection state | Preserve existing reset, timeout, and logging behavior. |

## Role Guards

Not applicable. This is a library/OS transport change with no routes, users, or
authorization roles. The equivalent guard is platform gating: Win32 ownership and
the production live provider stay behind `#[cfg(windows)]`, while the generic reader
logic and its tests remain portable.

## Failure Modes and Recovery

| Failure | User-visible behavior | Recovery |
| --- | --- | --- |
| iRacing not connected | Existing provider polling/logging continues; finite attempt policy can end with `Ok(None)` | Start or enter an iRacing session; provider resumes when status becomes connected. |
| Malformed/hostile header or out-of-range descriptor | Structured `IRacingSDKError` with parse/range context; no pointer arithmetic outside the mapped extent | Reconnect/restart; logs identify field/offset. |
| Producer advances during copy | Retry once, trace contention, then skip this observation without publishing mixed data | Next event/poll retries naturally. |
| Tick regresses after simulator reset | Reset baseline and do not publish the regressed stale frame | Publish on the next advancing tick. |
| Session changes while YAML is copied | Retry/return the stable current snapshot; never return a torn string | A later frame/session observation triggers another fetch. |
| Windows mapping/event failure | Existing Windows API error with operation name | Verify simulator state/permissions and reconnect. |
| Negative tick/session version at provider boundary | Parse/type-conversion error; do not wrap to a large `u32` | Treat as corrupt/transient input and retry through existing provider error policy. |

## Risks and Mitigations

| Risk | Likelihood | Mitigation |
| --- | --- | --- |
| Current Rust wire structs use undocumented padding as synchronization fields | High | Make installed-SDK ABI comparison and a captured header mandatory before implementation; add size/offset tests derived from the confirmed header. |
| A full header can change while it is copied | Medium | Validate before use and reread the minimal proof fields after copying the selected region; scripted tests mutate on exact read boundaries. |
| Tick wraparound makes ordinary signed ordering ambiguous | Low | Confirm producer type/reset behavior in SDK source and document the comparison rule; add boundary tests near `i32::MAX`. |
| Extra header reads or validation regress the 360 Hz hot path | Low | Benchmark the 8,586-byte captured layout, unchanged-tick fast path, accepted copy, and forced retry separately. |
| Generic test source behaves differently from real mmap writes | Medium | Pair portable deterministic tests with Windows live correlation against the frame's `SessionTick` variable and a sustained runtime smoke test. |
| `Option` to `Result<Option<_>>` changes public callers | High | Update every bin/example/provider in one atomic phase and run examples/bins/docs gates on macOS and Windows. |
| Session version changes independently of telemetry ticks | Medium | Capture session version with each accepted frame and separately stabilize YAML copies; retain and document the existing limitation that overwritten intermediate YAML cannot be recovered. |
| Cross-target all-target checks fail on macOS due C dev-dependency/MSVC headers | Medium | Use `cargo check --target x86_64-pc-windows-msvc --lib` as the macOS cfg gate; reserve all-target Windows test/bench builds for the Windows phase. |

## Definition of Done

### User Stories

- As a live telemetry consumer, I receive frame bytes whose packet tick was accepted
  by the same acquisition attempt, so decoded data is not labelled with a later
  frame's metadata.
- As a maintainer on macOS, I can deterministically test stable reads, retries,
  malformed ranges, resets, and session changes without iRacing or Win32.
- As a Windows maintainer, I can correlate packet ticks with `SessionTick`, inspect
  retry/drop diagnostics, and measure acquisition separately from simulator/event
  pacing.
- As a library caller, I receive actionable errors for malformed mapped-memory reads
  rather than an indistinguishable `None` or panic.

### Acceptance Criteria

1. No safe API in `windows/connection.rs` performs unchecked offset-based reads or
   returns a mapping-backed reference.
2. One `LiveFrameSnapshot` owns the complete frame and the tick/session version from
   its accepted attempt; `LiveProvider` performs no second header lookup.
3. Every advertised region is checked against the mapped extent before copying.
4. The unchanged-tick path allocates no frame buffer, and contention never publishes
   a mixed snapshot.
5. ABI layout tests match the SDK installed for the Windows validation run and record
   the source/version used.
6. Portable tests cover stable, unchanged, new, reset, retry-success,
   retry-exhaustion, invalid-header, out-of-bounds, tick-boundary, and session-change
   cases.
7. During a Windows live smoke run, every published packet satisfies
   `packet.tick == decoded SessionTick` when that variable is present; duplicates and
   regressions are zero outside a deliberately observed simulator reset. Gaps are
   reported, not treated as corruption, because live delivery is latest-wins.
8. Deterministic benchmarks report direct-copy floor, unchanged-tick fast path,
   stable accepted acquisition, and one-retry acquisition for the checked-in
   8,586-byte live layout. Any stable-path regression greater than 10% against the
   pre-change same-machine baseline requires explanation and reviewer approval.
9. Windows manual diagnostics report acquisition-only p50/p95/p99 and retry counts
   separately from event wait and end-to-end subscription latency. Acquisition p99
   must remain below 1% of the native frame period on the validation machine.
10. Rustdoc, examples, benchmark documentation, and telemetry architecture describe
    the implemented ownership, retry, reset, and session limitations.

### Validation Commands

macOS, after each portable phase:

```text
python3 scripts/check_test_fixtures.py
cargo fmt --all -- --check
cargo test -p iracing-sdk --lib reader::
cargo test -p iracing-sdk --all-targets
cargo clippy -p iracing-sdk --all-targets --all-features -- -D warnings
cargo check -p iracing-sdk --examples --bins
cargo test -p iracing-sdk --doc
RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps
cargo check -p iracing-sdk --target x86_64-pc-windows-msvc --lib
cargo bench -p iracing-sdk --features benchmark --bench live_reader_acquisition
```

Windows, before merge:

```text
python scripts/check_test_fixtures.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings
cargo test --workspace --all-targets
cargo test -p iracing-sdk --doc
set RUSTDOCFLAGS=-D warnings && cargo doc -p iracing-sdk --no-deps
cargo check -p iracing-sdk --examples --bins
cargo bench -p iracing-sdk --features benchmark --bench live_reader_acquisition
cargo bench -p iracing-sdk --features benchmark --bench live_frame_latency
```

Run ignored simulator-required tests explicitly with an active session and one test
thread so they do not contend for shared state:

```text
cargo test -p iracing-sdk --lib -- --ignored --test-threads=1
```

### Smoke Tests

- macOS: run the scripted-source test that advances the chosen descriptor during
  the first frame copy and remains stable on the second. Observe exactly one accepted
  snapshot containing second-attempt bytes, tick, and session version.
- Windows ABI: dump and inspect one live header, compare it to the installed SDK C
  definitions, and verify every mapped range before enabling streaming.
- Windows live: collect at least 10 minutes spanning garage-to-track and one session
  metadata update. Assert packet tick/`SessionTick` correlation, parse every captured
  YAML snapshot, and report accepted frames, tick gaps, retries, contention skips,
  resets, and errors.
- Disconnect/reconnect: stop or leave the session, observe baseline reset without a
  stale frame, reconnect, and verify advancing frames resume without process restart.

## Phased Delivery

| Phase | Goal | Key outputs |
| --- | --- | --- |
| 0 | Establish build and ABI truth | Windows cfg build repaired; installed SDK/header evidence; characterization decisions locked. |
| 1 | Implement portable live acquisition | `LiveReader`, owned snapshots, scripted/property tests, deterministic benchmark. |
| 2 | Integrate the Windows transport | Ephemeral `MappedView`, fallible delegated connection reads, no duplicate pointer/range logic. |
| 3 | Integrate provider and callers | One-pass packet metadata, stabilized session buffer, updated bins/examples/schema construction. |
| 4 | Validate and document on Windows | Live correlation/smoke results, manual benchmark report, CI/docs gates, rollout decision. |

### Phase 0 — Baseline and ABI Verification

Status: Not started

- [ ] `crates/iracing-sdk/src/providers/live/mod.rs` — criterion: the current branch
  passes `cargo check -p iracing-sdk --target x86_64-pc-windows-msvc --lib` before
  reader behavior is changed.
- [ ] `crates/iracing-sdk/src/types/irsdk/header.rs` — criterion: each field name,
  offset, and size is reconciled with the current installed iRacing SDK header; bytes
  documented as padding are not treated as synchronization state.
- [ ] `crates/iracing-sdk/src/types/irsdk/variable_buffer.rs` — criterion: descriptor
  fields and padding match the same SDK version and have explicit layout tests.
- [ ] `crates/iracing-sdk/src/reader/live.rs` — criterion: characterization tests lock
  first observation, same tick, increasing tick, decreasing/reset tick, and newest
  descriptor selection according to the confirmed SDK client behavior.
- [ ] `docs/plans/live-reader-integration.md` — criterion: the ABI source/version and
  final protocol choice are recorded before Phase 1 starts.

### Phase 1 — Portable Live Reader

Status: Not started

- [ ] `crates/iracing-sdk/src/reader/live.rs` — criterion: `LiveReader` owns acquisition
  state and accepts any `RandomAccessSource` per call without Win32 dependencies.
- [ ] `crates/iracing-sdk/src/reader/live.rs` — criterion: `LiveFrameSnapshot` owns frame
  bytes, tick, and session version from one accepted before/copy/after attempt.
- [ ] `crates/iracing-sdk/src/reader/live.rs` — criterion: `LiveSessionSnapshot` owns YAML
  bytes and its actual stable version after checking version/offset/length across the
  copy.
- [ ] `crates/iracing-sdk/src/reader/live.rs` — criterion: malformed headers and ranges
  return errors, unchanged ticks avoid frame allocation, one concurrent change retries,
  and two concurrent changes publish nothing.
- [ ] `crates/iracing-sdk/src/reader/live.rs` — criterion: proptests over signed counts,
  offsets, lengths, descriptor ticks, and source extents never panic or read outside the
  source.
- [ ] `crates/iracing-sdk/benches/live_reader_acquisition.rs` — criterion: macOS can
  measure direct-copy, unchanged, stable-new, and one-retry cases using the captured
  8,586-byte layout.
- [ ] `crates/iracing-sdk/Cargo.toml` — criterion: the portable benchmark is registered
  behind `benchmark` and compiles with `--no-run`.
- [ ] `crates/iracing-sdk/benches/README.md` — criterion: timed boundaries, throughput
  units, allocation behavior, and non-goals are documented.

### Phase 2 — Windows Connection Delegation

Status: Not started

- [ ] `crates/iracing-sdk/src/windows/connection.rs` — criterion: connection setup owns
  mapping/event lifetime and produces only a lifetime-bound `MappedView<'_>` for a
  reader call.
- [ ] `crates/iracing-sdk/src/windows/connection.rs` — criterion: header, metadata,
  session, and frame methods delegate to reader/header abstractions and return
  structured `Result` values.
- [ ] `crates/iracing-sdk/src/windows/connection.rs` — criterion: duplicate raw pointer
  helpers, range arithmetic, buffer selection, and `expect`-based wire parsing are
  removed.
- [ ] `crates/iracing-sdk/src/windows/connection.rs` — criterion: disconnect/reset and
  event-wait behavior remain characterized and tracing distinguishes no-new-data,
  retry, invalid data, and OS failure.
- [ ] `crates/iracing-sdk/src/reader/live.rs` — criterion: `MappedView` safety docs state
  the mapping owner/lifetime and concurrent-writer assumptions used by `Connection`.

### Phase 3 — Provider and Caller Migration

Status: Not started

- [ ] `crates/iracing-sdk/src/providers/live/mod.rs` — criterion: `next_frame_impl`
  constructs a packet only from one `LiveFrameSnapshot` and never rereads header or
  buffer selection metadata.
- [ ] `crates/iracing-sdk/src/providers/live/mod.rs` — criterion: session acquisition
  consumes a stable owned reader snapshot, converts through `IRacingSessionString`,
  and documents behavior when the actual current version differs from the trigger.
- [ ] `crates/iracing-sdk/src/types/schema.rs` — criterion: live schema construction uses
  reader-owned variable headers and the validated frame length, parallel to
  `VariableSchema::from_reader(&IbtReader)`.
- [ ] `crates/iracing-sdk/src/bin/` — criterion: every live binary compiles against the
  fallible snapshot API and reports actionable errors.
- [ ] `crates/iracing-sdk/examples/` — criterion: every live example compiles and shows
  the new error/snapshot handling on its happy path.
- [ ] `crates/iracing-sdk/README.md` — criterion: direct `WindowsConnection` usage, if
  retained publicly, matches the implemented API and ownership guarantees.
- [ ] `docs/architecture/telemetry-pipeline.md` — criterion: the documented live path
  exactly matches one owned reader snapshot through provider packet construction and
  no longer describes stale borrowed/split reads.

### Phase 4 — Windows Runtime, Performance, and Release Validation

Status: Not started

- [ ] `crates/iracing-sdk/src/windows/connection.rs` — criterion: ignored live tests pass
  serially with an active simulator and include disconnect/reconnect coverage.
- [ ] `crates/iracing-sdk/benches/live_frame_latency.rs` — criterion: acquisition-only
  p50/p95/p99, retries, skipped inconsistent observations, tick gaps, and
  packet/`SessionTick` mismatches are reported separately from waits and subscription
  setup.
- [ ] `crates/iracing-sdk/benches/live_frame_latency.rs` — criterion: the 10-minute smoke
  run reports zero packet/`SessionTick` mismatches, duplicates, and unexplained tick
  regressions.
- [ ] `crates/iracing-sdk/benches/README.md` — criterion: environment capture and
  before/after comparison instructions make benchmark results reproducible.
- [ ] `docs/plans/live-reader-integration.md` — criterion: Phase 4 records the Windows
  SDK/simulator version, hardware, command outputs, benchmark summary, known gaps, and
  go/no-go result in a dated immutable audit file.
- [ ] `.github/workflows/quality.yml` — criterion: no workflow change is required unless
  the new portable tests/benchmark reveal a missing existing gate; Ubuntu and Windows
  quality jobs pass before merge.

## Rollout and Rollback

Deliver one commit per phase and keep the provider migration separate from the
portable reader implementation. Merge only after the Windows live smoke criteria pass;
there is no feature flag because two acquisition paths would duplicate the unsafe and
consistency-sensitive boundary.

If Windows validation finds ABI or consistency mismatches, stop rollout and revert the
connection/provider delegation commit while retaining portable tests and the recorded
ABI evidence for correction. If performance alone misses the stated budget, keep the
correct reader path, profile header copies/validation and allocation sites, and require
a measured follow-up rather than restoring split metadata reads.
