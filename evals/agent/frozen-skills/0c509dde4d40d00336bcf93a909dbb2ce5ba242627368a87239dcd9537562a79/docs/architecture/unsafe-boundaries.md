# Trusted unsafe boundaries

This is a map of obligations in current source, not a soundness certificate.
Use `.agents/skills/rust-soundness-review/` to audit complete producer/consumer
paths. Proposed `MappedView`/RAII handle wrappers are not implemented on main;
the current live boundary is `windows::Connection`.

## Live shared memory

[`Connection`](../../crates/iracing-sdk/src/windows/connection.rs) opens a mapping,
maps its whole view and opens an update event. Successful construction retains
handles and a `NonNull<u8>`; Drop unmaps/closes them. Partial-construction failure
paths need separate cleanup review. The type stores no mapped byte extent.
`header`, `get_new_data` and `ByteParser::bytes_at_region` form Rust references
from pointers. Non-nullness alone does not prove extent, alignment, valid header
contents or freedom from external mutation. OS page alignment does not establish
all dynamically offset accesses. Header-advertised ranges need mapping bounds
and fresh cross-field validation before pointer arithmetic.

The simulator is a separate writer; FILE_MAP_READ describes this process's
permissions and does not make that memory immutable. Manual Send/Sync currently
justify handles and a pointer as read-only. A complete proof must also cover
shared access, external writes, reference validity and destruction. Async waits
copy the event handle into spawn_blocking; cancellation can outlive the borrowed
future, so the worker's handle lifetime is an independent obligation.

The current frame method reads ticks around *forming a borrowed slice*, not an
owned copy, using ordinary loads. It has no volatile accesses or explicit
compiler/hardware read barriers. The returned bytes can change after the tick
check; do not describe this as a stable owned snapshot. Resolving these live
obligations belongs to live-source work, not this documentation change.

For future raw read implementations, volatile access makes individual accesses
observable; it does not provide Rust atomic synchronization, prevent data races
or prove a coherent multi-byte snapshot. Compiler fences constrain compiler
reordering and hardware barriers constrain specified CPU ordering; neither
proves source extent/lifetime or an external producer's update protocol. An
owned copy plus protocol-aware metadata/tick rechecks must establish snapshot
consistency above raw source access. An update event alone is not a snapshot lock.

## Disk mappings

[`IbtReader::open`](../../crates/iracing-sdk/src/ibt/reader/mod.rs) maps completed
recordings read-only. The backing file must remain unchanged/untruncated by any
process until the reader is dropped. The mapping owns its view independently of
the File handle. This is an external storage assumption, not a fact proved by
opening a file read-only or by validating IBT headers. `from_bytes` owns its
storage when that assumption cannot be guaranteed. Layout validation cannot
repair mmap lifetime/storage violations.

## Wire contracts

[`iracing-irsdk`](../../crates/iracing-irsdk/src/lib.rs) owns fixed ABI types.
repr(C)/transparent plus size/alignment/offset checks establish geometry.
zerocopy FromBytes admits all bit patterns for that type; constrained enums use
checked decoding. Exact byte-size validation does not imply meaningful lengths,
counts, enum sentinel acceptability, cross-field layout, source bounds or a
stable live snapshot. Wire-to-domain conversion owns semantic checks; the source
and layout layer own actual extent and geometry. Preserve runtime round trips.

## Range and layout contracts

Current main uses [`ByteRegion`](../../crates/iracing-sdk/src/types/regions/bytes.rs),
not ByteRange. Its constructors prove a representable non-overflowing exclusive
end and reject reversed ranges. They do not bind the region to any source.
Specialized regions validate additional geometry; inspect each constructor.
[`IbtLayout`](../../crates/iracing-sdk/src/types/ibt/layout.rs) validates physical
geometry against a particular source length. It does not validate variable
semantics or the lifetime of externally mutated storage. Readers must enforce
exact-read destinations and source bounds; providers own schema/session meaning.
