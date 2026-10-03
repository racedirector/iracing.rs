# Trusted unsafe boundaries

This is a map of obligations in current source, not a soundness certificate.
Use `.agents/skills/rust-soundness-review/` to audit complete producer/consumer
paths.

## Live shared memory

[`LiveSource`](../../crates/iracing-sdk/src/windows/source.rs) owns the mapping,
view and event through RAII wrappers. It retains a mapped extent from
`VirtualQuery`; activation requires the complete fixed header. Scalar reads use
bounded volatile loads, and native `RtlMoveMemory` copies into disjoint owned
storage without constructing Rust references into simulator memory. Header,
session-info and exact typed variable-header snapshots remain stable after
copying; a header copy alone is not an atomic multi-field observation.

[`Connection`](../../crates/iracing-sdk/src/windows/connection.rs) validates
positive live frame length and signed slot offsets through `FrameRegion`, then
independently proves containment in the retained mapping before copying. It
retains native current-buffer selection and two attempts. Tick, session version
and selected-slot geometry checks bracket the copy; acceptance returns owned
bytes and matching metadata together as `LiveFrameSnapshot`. `LiveProvider`
consumes it into `FramePacket` without a second copy or shared-header read.
No safe borrowed live header or mapped byte slice escapes this boundary.

The simulator is a separate writer. FILE_MAP_READ describes this process's
permissions, not immutability. Volatile reads make scalar observations visible;
they do not make the whole header atomic. Snapshot acceptance depends on the
native producer's tick publication protocol and version counters, including
that a transition is not hidden by counter reuse during an attempt. An update
event alone is not a snapshot lock. Manual Send/Sync depends on retaining the
mapping throughout reads and copies. Async blocking waits own an Arc to the
event, so cancellation cannot close the handle while the worker waits.

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
