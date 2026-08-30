# Reader Simplification Refactor Plan

Status: Implemented on 2026-08-30; active-simulator smoke validation remains
environment-dependent

## Implementation Result

The source-neutral layers were removed. `IbtReader` now directly owns immutable
bytes, parsed headers, checked metadata/frame geometry, and one cursor.
`LiveReader` is fallible, binds itself to one mapping base/extent, stores all
static frame and variable-header regions, and keeps the normal frame path to
volatile dynamic controls plus the SDK barrier/copy/barrier proof. The Windows
connection owns one reader per mapping generation, and `LiveProvider` constructs
packets from the accepted owned snapshot without rereading metadata.

Same-machine Criterion medians from the Phase 0 and final runs:

| Case | Phase 0 | Final | Absolute change |
| --- | ---: | ---: | ---: |
| Direct 8,586-byte copy | 206.38 ns | 116.62 ns | -43.5% |
| Unchanged tick | 56.086 ns | 5.4142 ns | -90.3% |
| Stable accepted frame | 1.7160 us | 1.8324 us | +6.8% |
| One retry | 3.3755 us | 3.3311 us | -1.3% |

Criterion's distribution comparison reported no statistically detectable change
for the stable-frame case (`p = 0.99`, estimated change centered at -0.05%); its
confidence interval crossed both improvement and regression. Inspection
confirmed that the accepted hot path performs one output allocation/copy and no
layout reconstruction. The unchanged path improved by about 10x and performs
no frame copy or output allocation.

Portable tests, Windows compilation, and the ignored packet/`SessionTick`
correlation test are present. The ignored correlation test reached the local
mapping but received no accepted frame during its 12-second probe, so a
sustained active-session run covering frames, session replacement, disconnect,
and reconnect could not be completed. The design and test were not weakened to
simulate that final environment check.

## Objective

Refactor `crates/iracing-sdk/src/reader/` so each source-specific reader is the
single parsed and validated boundary for its source. Restore the directness of
the former `ibt::reader::IbtReader` while retaining the stronger wire-layout,
bounds, ownership, and live-concurrency guarantees developed on the
`feature/wire-format` branch.

The intended reader API has two concrete entry points:

```text
reader::ibt::IbtReader
reader::live::LiveReader
```

Construction performs structural parsing and validation. Successful
construction establishes the invariants used by later reads. Routine methods do
only the work required by their source: cursor arithmetic and copying for IBT,
and dynamic control-word reads plus the SDK consistency protocol for live
telemetry.

This plan supersedes the source-neutral `RandomAccessSource` and
`HeaderSnapshotReader` direction recorded in
`docs/plans/live-reader-integration.md`. The live behavior, ownership, retry,
reset, and provider-boundary decisions from that plan remain applicable unless
this document explicitly replaces them.

## Design Principles

1. **Parse, do not repeatedly validate.** Convert raw signed wire fields into
   usable internal values once. A constructed reader represents a source whose
   structural layout has already been accepted.
2. **The concrete reader carries the invariant.** Do not add a public
   `ValidatedHeader`, `ValidatedLayout`, recording wrapper, or generic source
   framework beside the reader.
3. **Keep source semantics separate.** IBT bytes are immutable; live shared
   memory is concurrently updated. Share small value helpers only when they
   preserve that distinction.
4. **Use newtypes at semantic boundaries.** Retain `FrameBuffer`,
   `SessionInfoBuffer`, and `VariableHeadersBuffer`. A small private checked
   region type is acceptable if it makes unchecked internal copies locally
   provable.
5. **Keep the hot path visibly small.** A reviewer should be able to read
   `LiveReader::next_frame` from top to bottom and identify every operation
   performed per observation.
6. **Make unsafe ordering explicit.** Mapped-memory control reads, barriers, and
   copies belong together at the `MappedView` boundary.
7. **Fix callers rather than preserve obsolete structure.** Do not add
   compatibility aliases or forwarding wrappers for removed reader layers.

## Current Problems

The current IBT path distributes one reader's responsibilities across:

```text
IbtReader
  -> IbtRecording
     -> HeaderSnapshotReader
        -> RandomAccessSource
```

The current live path distributes acquisition across:

```text
LiveReader
  -> Header
  -> HeaderRegions
  -> HeaderSnapshotReader
  -> RandomAccessSource
  -> MappedView
```

This layering creates several practical problems:

- successful construction does not communicate all later read guarantees;
- ranges and signed fields are reconstructed after they have already been
  checked;
- `RandomAccessSource::snapshot` performs a bounds check for every copy;
- live frame acquisition validates every advertised region on every attempt;
- the generic byte-source contract cannot express the ordering required by
  concurrently written shared memory;
- readers expose intermediate abstractions that callers do not need;
- understanding one frame read requires crossing several files and contracts.

## Scope

In scope:

- simplify all implementation files under `crates/iracing-sdk/src/reader/`;
- make `IbtReader` directly own immutable source bytes and validated layout;
- make `LiveReader` directly own validated static live layout and acquisition
  state;
- isolate mapped-memory reads and ordering in `MappedView`;
- eliminate repeated structural validation from normal frame reads;
- preserve owned snapshot outputs;
- preserve IBT cursor, seeking, header, session, and variable-header behavior;
- preserve live current-buffer selection, tick baseline/reset behavior,
  two-attempt torn-read handling, and session stabilization;
- update repository callers, tests, benchmarks, and documentation affected by
  removal of reader intermediates;
- measure the unchanged-tick and accepted-frame paths against the current
  implementation.

Out of scope:

- changing provider delivery policy or subscription behavior;
- changing IBT replay acknowledgement semantics;
- making live delivery lossless;
- changing telemetry decoding, schema contents, or YAML preprocessing;
- replacing Windows mapping or event-wait APIs;
- introducing a shared high-level reader trait;
- retaining generic custom-source support as public API without a demonstrated
  consumer;
- treating session YAML offset and length as permanently immutable;
- redesigning `FrameBuffer`, `SessionInfoBuffer`, or
  `VariableHeadersBuffer`.

## Target Directory Structure

```text
crates/iracing-sdk/src/reader/
  mod.rs       module documentation and, if useful, one private checked-region helper
  ibt.rs       complete immutable IBT parser, validated layout, and cursor
  live.rs      complete live layout parser, mapped-memory boundary, and acquisition state
```

Expected removals:

```text
crates/iracing-sdk/src/reader/access_source.rs
crates/iracing-sdk/src/reader/header.rs
```

If a checked-region helper is shared, it must remain private to `reader`. It
should contain only an offset and length/end whose arithmetic and source
containment were proven at construction. It must not grow source I/O methods or
become another reader layer.

## Target IBT Design

### Ownership

`IbtReader` directly owns its immutable source and all derived layout:

```rust,ignore
pub struct IbtReader {
    bytes: Arc<[u8]>,
    header: IbtHeader,
    session_region: Option<CheckedRegion>,
    variable_headers_region: Option<CheckedRegion>,
    frame_data_start: usize,
    frame_length: usize,
    total_frames: usize,
    next_frame: usize,
    path: Option<PathBuf>,
}
```

The exact private fields may vary, but there must be only one public ownership
type. Remove `IbtRecording` unless a concrete repository consumer requiring
multiple independent cursors over one shared recording is identified before
implementation.

### Construction

`open`, `from_bytes`, and `from_reader` converge on one private constructor. It:

1. materializes immutable bytes;
2. copies and parses the common and disk headers;
3. validates wire version and IBT-specific scalar rules;
4. converts metadata offsets, counts, and lengths to `usize` once;
5. validates metadata regions against the complete source;
6. computes the first frame offset;
7. validates frame length, complete-frame divisibility, and record count;
8. stores the resulting regions and counts.

No later IBT read re-runs header validation, signed conversion, region
construction, or source containment checks already implied by the reader.

### Read behavior

- `frame(index)` checks only `index < total_frames`, derives the already-safe
  frame offset, and copies exactly `frame_length` bytes.
- `read_next_frame` calls the same private frame-copy operation and advances
  only `next_frame` after success.
- `seek_to_frame` validates the requested index and updates only `next_frame`.
- session and variable-header methods copy their stored optional regions.
- `tick_rate`, duration, and header access use the parsed header directly; do
  not apply fallback values to fields already accepted during construction.

The reader must not retain both a byte cursor and a frame cursor.

## Target Live Design

### Static and dynamic data

The live reader distinguishes static mapping layout from dynamic producer state.

Validated once for one mapping generation:

- SDK version and supported live scalar limits;
- mapped extent;
- buffer count and frame length;
- each advertised frame-buffer offset and complete region;
- variable-header offset and complete region;
- descriptor/control-word offsets used by acquisition.

Read dynamically when required:

- connection status;
- current buffer index;
- selected descriptor's `tickCount` and `tickCountBegin`;
- session information update counter;
- session information offset and length.

Session offset and length remain dynamic because the producer exposes one
mutable current YAML region. They are checked on the slower session-update path,
not on every frame observation.

### Reader state

`LiveReader` directly stores its validated layout and acquisition state:

```rust,ignore
pub struct LiveReader {
    mapped_length: usize,
    buffer_count: usize,
    frame_regions: [Option<CheckedRegion>; Header::MAX_BUFFERS],
    variable_headers_region: Option<CheckedRegion>,
    last_tick_count: Option<i32>,
    diagnostics: LiveReaderDiagnostics,
}
```

This is illustrative rather than a required public field layout. There is no
separate public live-layout object. If source identity must be checked because
an ephemeral `MappedView` is passed to each method, store only the minimal
mapping identity needed to reject use with a different mapping generation.

### Construction and reset

`LiveReader::new` becomes fallible and reads the initial header through a
`MappedView`. It parses and validates all static regions before returning.

The owner must construct a new `LiveReader` whenever the mapping is opened,
reopened, resized, or replaced. `reset` clears only tick-baseline state and does
not claim to validate a new mapping. This mapping-generation contract must be
documented at both `LiveReader` and the eventual Windows `Connection`
integration point.

### Frame hot path

After construction, one normal `next_frame` observation performs only:

1. ordered read of connection status;
2. ordered read and range check of current buffer index;
3. ordered read of the selected descriptor's completed tick;
4. tick baseline/new/unchanged/reset classification;
5. for a new tick, ordered copy of the selected prevalidated frame region;
6. ordered read of the descriptor's `tickCountBegin`;
7. equality check, acceptance, or one retry.

The hot path must not:

- copy and validate a complete `Header`;
- rebuild metadata or frame regions;
- iterate over inactive buffer descriptors;
- validate session or variable-header regions;
- convert signed layout fields;
- check a prevalidated frame region against mapped length again;
- allocate when the tick is unchanged.

The current buffer index remains checked each observation because it is dynamic.
That one check selects from the reader's prevalidated array and is part of the
protocol rather than repeated structural validation.

### Mapped-memory boundary

`MappedView` remains the narrow unsafe boundary and gains the explicit
operations needed by live acquisition:

- ordered/volatile reads of aligned control words;
- a documented read barrier matching the SDK 1.20 `_ReadBarrier` role;
- copying a prevalidated region into owned bytes;
- checked copying for dynamic session regions;
- mapped base/extent identity where needed.

The production frame sequence must be visibly equivalent to the local SDK 1.20
implementation:

```text
read tickCount
read barrier
copy complete frame
read barrier
read tickCountBegin
accept only when equal
```

The precise Rust/Windows primitive must be confirmed before implementation.
Control-word volatility and compiler/CPU ordering are separate concerns and
must be documented separately. Unsafe code must not be hidden behind the former
source-neutral trait.

### Session path

`session_info` remains a checked slow path:

1. read update, offset, and length;
2. parse and validate that dynamic region against mapped extent;
3. copy the region;
4. apply the required ordering boundary;
5. reread update, offset, and length;
6. accept only if the proof is unchanged, otherwise retry once.

This work occurs when session information is requested or its version changes;
it is not part of unchanged or accepted frame acquisition.

### Variable headers

Variable-header layout is accepted during `LiveReader` construction. Copy it
from its stored region without re-reading or re-validating the complete header.
If live evidence shows variable metadata changes without mapping replacement,
that evidence must be recorded and the reader reconstructed through an explicit
slow path; do not silently add a per-frame layout signature.

## Public API Decisions

Target public reader surface:

```text
reader::ibt::IbtReader
reader::ibt::IbtFrame
reader::live::LiveReader
reader::live::LiveFrameSnapshot
reader::live::LiveSessionSnapshot
reader::live::LiveReaderDiagnostics
reader::live::MappedView (only if required by the Windows boundary)
```

Remove or make private:

```text
reader::access_source::RandomAccessSource
reader::access_source::OwnedBytes
reader::access_source::ByteRegion
reader::header::HeaderRegions
reader::header::HeaderSnapshotReader
reader::ibt::IbtRecording
generic source parameters on IbtReader and LiveReader methods
```

Before deletion, run a repository-wide reference check and allow compiler
failures to identify missed internal callers. Do not retain aliases solely to
keep old paths compiling.

## Implementation Phases

### Phase 0 — Characterize behavior and capture baselines

- [ ] Record current focused reader test results.
- [ ] Run the current deterministic live acquisition benchmark and save
  unchanged-tick, stable-frame, and one-retry measurements from the same
  machine/toolchain used after the refactor.
- [ ] Add or confirm characterization tests for every public behavior that must
  survive the structural change.
- [ ] Confirm whether any repository or published API requirement genuinely
  needs `IbtRecording`, generic sources, or independently shareable cursors.
- [ ] Confirm the ordering primitive used to reproduce the SDK 1.20 read
  barriers on supported Windows targets.

Exit criterion: behavior and performance baselines exist, and no unresolved
consumer requires one of the layers scheduled for removal.

### Phase 1 — Collapse the IBT reader

- [ ] Move immutable byte ownership and validated layout into `IbtReader`.
- [ ] Replace generic source reads with direct immutable slice copies.
- [ ] Store checked metadata regions and frame geometry at construction.
- [ ] Remove the second cursor (`current_position`) if introduced during
  migration; derive offsets from `next_frame`.
- [ ] Preserve owned `IbtFrame` output and existing header/path/time methods.
- [ ] Remove `IbtRecording` and repair all repository callers directly.
- [ ] Add constructor tests proving malformed layout is rejected before any
  frame method is available.
- [ ] Add read-count or invariant tests proving later reads do not reparse or
  revalidate the header.

Exit criterion: the IBT implementation is one concrete reader, all IBT tests
pass, and no source-neutral reader type is needed by the IBT path.

### Phase 2 — Make mapped-memory ordering explicit

- [ ] Add aligned volatile control-word reads to `MappedView`.
- [ ] Add the confirmed read-barrier primitive with focused safety comments.
- [ ] Separate prevalidated frame copies from checked dynamic-region copies.
- [ ] Express the SDK read/barrier/copy/barrier/read sequence directly in
  `LiveReader::next_frame`.
- [ ] Preserve deterministic contention tests through a private test seam or
  another non-public mechanism.
- [ ] Add Windows-only validation that accepted ticks correlate with decoded
  `SessionTick`.

Exit criterion: no accepted live frame relies on ordinary unordered scalar
reads, and the unsafe ordering contract is contained in `MappedView`.

### Phase 3 — Parse the live layout into LiveReader

- [ ] Make `LiveReader` construction fallible and source-aware.
- [ ] Validate and store static frame and variable-header regions once.
- [ ] Remove complete-header and advertised-region validation from
  `next_frame`.
- [ ] Keep current-buffer bounds checking and torn-read proof in the hot path.
- [ ] Keep session region parsing and proof on the session slow path.
- [ ] Define and test mapping replacement/reconstruction behavior.
- [ ] Update diagnostics so retries, contention skips, and resets retain their
  existing meanings.

Exit criterion: the unchanged-tick path performs only dynamic control reads and
classification, and the accepted path adds one owned frame copy plus its proof.

### Phase 4 — Remove the source-neutral layers

- [ ] Move any remaining small checked arithmetic helper into `reader/mod.rs` or
  its sole source-specific caller.
- [ ] Delete `access_source.rs`.
- [ ] Delete `header.rs`.
- [ ] Remove module exports and stale documentation.
- [ ] Update the live acquisition benchmark to exercise the concrete reader
  path without exposing a production test-source trait.
- [ ] Run compiler-driven migration across providers, connections, bins,
  examples, tests, and benchmarks.

Exit criterion: production reader code contains no generic byte-source or
header-snapshot layer, and each frame path is understandable within its
source-specific file.

### Phase 5 — Verify performance and integration

- [ ] Re-run deterministic benchmarks on the Phase 0 machine/toolchain.
- [ ] Compare unchanged-tick, stable-frame, and retry costs with the baseline.
- [ ] Verify the unchanged path allocates zero frame buffers.
- [ ] Verify an accepted frame allocates only its owned output buffer unless a
  documented reusable-buffer design is adopted separately.
- [ ] Run a sustained Windows live smoke test, including disconnect/reconnect
  and at least one session update.
- [ ] Update architecture documents and the older integration plan to reflect
  the implemented concrete-reader design.

Exit criterion: correctness gates pass, the hot path is measurably no slower
than the pre-refactor baseline, and any regression is explained with evidence.

## File-by-File Work Map

| File | Planned action |
| --- | --- |
| `reader/mod.rs` | Replace source-neutral architecture documentation with the two concrete reader contracts; optionally hold one private checked-region helper. |
| `reader/ibt.rs` | Collapse `IbtRecording`, `OwnedBytes`, regions, layout validation, and cursor into one concrete `IbtReader`. |
| `reader/live.rs` | Store validated static layout in `LiveReader`; put ordered mapped access in `MappedView`; reduce `next_frame` to the SDK protocol. |
| `reader/access_source.rs` | Delete after both readers stop depending on it. |
| `reader/header.rs` | Delete after header-to-region parsing moves into each reader's constructor/slow path. |
| `windows/connection.rs` | In the integration phase, construct/reconstruct `LiveReader` per mapping generation and delegate reads without duplicating pointer logic. |
| `providers/ibt/mod.rs` | Update direct `IbtReader` use if removed intermediate methods surface compile failures. |
| `providers/live/` | Consume accepted snapshots; do not reread header metadata. |
| `types/schema.rs` | Continue constructing schemas from owned variable-header snapshots without depending on reader internals. |
| `benches/live_reader_acquisition.rs` | Benchmark the concrete reader and report control-path versus copy costs. |
| `docs/architecture/telemetry-pipeline.md` | Replace `IbtRecording` and source-neutral layering with concrete reader invariants. |
| `docs/plans/live-reader-integration.md` | Mark superseded architecture decisions and retain only still-applicable integration history. |

## Validation Strategy

### IBT behavior

- valid fixture construction;
- malformed common/disk header rejection;
- negative and overflowing offsets/counts rejected during construction;
- metadata overlap and out-of-bounds rejection;
- partial final frame rejection;
- disk record-count mismatch rejection;
- first, middle, final, and EOF frame reads;
- seek to valid frame and EOF position;
- invalid seek rejection;
- session and variable-header snapshot ownership;
- no header/layout revalidation after construction.

### Live behavior

- valid layout construction;
- malformed and out-of-range layout rejection during construction;
- first observation establishes a baseline;
- unchanged tick performs no frame copy or frame allocation;
- stable new tick returns owned bytes and same-attempt metadata;
- current advertised descriptor is selected;
- one concurrent write retries and accepts the second attempt;
- two concurrent writes skip without advancing the baseline;
- disconnect and tick regression reset baseline behavior;
- invalid dynamic current-buffer index fails without unsafe access;
- session proof accepts stable bytes and retries changed bytes;
- arbitrary malformed construction inputs never issue out-of-bounds reads;
- mapping replacement requires reconstruction;
- ordered control-read/copy sequence is covered structurally and by Windows
  runtime correlation.

### Commands

During implementation, run focused gates after each phase:

```text
cargo test -p iracing-sdk --lib reader::
cargo test -p iracing-sdk --all-targets
cargo fmt --all -- --check
cargo clippy -p iracing-sdk --all-targets --all-features -- -D warnings
cargo check -p iracing-sdk --examples --bins
cargo test -p iracing-sdk --doc
RUSTDOCFLAGS="-D warnings" cargo doc -p iracing-sdk --no-deps
cargo bench -p iracing-sdk --features benchmark --bench live_reader_acquisition
```

Before completion, run the workspace quality gates from the repository
`AGENTS.md`, plus the ignored Windows live tests with one test thread and an
active simulator session.

## Performance Acceptance Criteria

For one unchanged live observation:

- no heap allocation;
- no frame-region copy;
- no complete-header parse or validation;
- no metadata-region validation;
- no loop over inactive buffers.

For one accepted stable frame:

- one owned frame allocation and copy;
- only the control reads and barriers required by the SDK consistency protocol;
- no repeated structural range validation.

For IBT frame reads:

- no header parse, schema parse, or metadata validation after construction;
- one index check, checked/established-safe offset derivation, and one owned
  frame copy.

Benchmark results must include absolute timing and relative change from the
Phase 0 same-machine baseline. Any regression greater than 5% in the
unchanged-tick or stable-frame acquisition benchmarks requires investigation;
greater than 10% requires an explicit design justification before merge.

## Completion Criteria

The refactor is complete when:

1. `reader/` exposes two concrete readers rather than a generic source-neutral
   framework.
2. A successfully constructed reader represents a structurally valid source
   layout.
3. `IbtReader` owns immutable bytes, validated metadata/frame geometry, and one
   frame cursor directly.
4. `LiveReader` owns validated static layout and acquisition state directly.
5. Live control-word reads and barriers are explicit at the mapped-memory
   boundary.
6. No routine frame path repeats structural validation established at
   construction.
7. Dynamic live fields remain checked at the point they are used.
8. Session YAML remains protected by before/copy/after proof checks.
9. Owned wire-output newtypes remain the reader/provider boundary.
10. Focused, workspace, documentation, Windows smoke, and performance gates
    pass.
11. A maintainer can understand either frame path by reading one source-specific
    reader file plus the wire type definitions it consumes.

## Risks and Guardrails

| Risk | Guardrail |
| --- | --- |
| Removing generic readers breaks an unknown downstream consumer. | Treat this as an intentional breaking refactor, audit repository uses, document the release impact, and do not preserve unused abstractions speculatively. |
| Cached live layout becomes stale. | Bind a reader to one mapping generation and reconstruct on mapping replacement/reconnect; keep session region dynamic. |
| Bounds checks are removed without a durable proof. | Store only privately constructed checked regions and keep unsafe copies adjacent to the invariant they rely on. |
| Compiler or CPU reorders live reads around the frame copy. | Use explicit volatile control reads and confirmed read barriers matching the SDK protocol; validate on Windows. |
| Simplification accidentally moves schema/YAML interpretation into acquisition. | Keep readers returning owned wire snapshots; providers and schema/session types retain interpretation responsibilities. |
| Tests force a public abstraction back into production. | Use a private test seam, internal benchmark harness, or mapped test storage rather than exporting a generic source trait. |
| IBT convenience behavior from the old reader is lost. | Characterize public behavior before deletion and migrate useful behavior directly into the concrete reader. |
