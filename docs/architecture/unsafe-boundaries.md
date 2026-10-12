# Trusted unsafe boundaries

This is a map of obligations in current source, not a soundness certificate.
Use `.agents/skills/rust-soundness-review/` to audit complete producer/consumer
paths.

## Live shared memory

The [primitive live source](../../crates/iracing-sdk/src/source/live.rs) owns
mapping/view/event lifetime, scalar access, native copying, unchecked caller
obligations, and cancellation-safe event retention. Follow its module/type docs,
local SAFETY comments, and private Windows mapping/event tests.

[`Connection`](../../crates/iracing-sdk/src/windows/connection.rs) validates
positive live frame length and signed slot offsets before constructing a `FrameRegion`, then
independently proves containment in the retained mapping before copying. It
retains native current-buffer selection and two attempts. Tick, session version
and selected-slot geometry checks bracket the copy; acceptance returns owned
bytes and matching metadata together as `LiveFrameSnapshot`. `LiveProvider`
consumes it into `FramePacket` without a second copy or shared-header read.
No safe borrowed live header or mapped byte slice escapes this boundary.

Snapshot acceptance depends on the native producer publication protocol and
version counters, including that a transition is not hidden by counter reuse
during an attempt. The connection owns that acceptance policy; primitive-source
contracts do not certify it.

## Disk mappings

[`IbtFile::open`](../../crates/iracing-sdk/src/ibt/file.rs) maps completed
recordings read-only. The backing file must remain unchanged/untruncated by any
process until the file or owning replay is dropped. The mapping owns its view independently of
the File handle. This is an external storage assumption, not a fact proved by
opening a file read-only or by validating IBT headers. `from_bytes` owns its
storage when that assumption cannot be guaranteed. Layout validation cannot
repair mmap lifetime/storage violations.

The file establishes owned variable/session snapshots and a validated shared
telemetry layout once. Metadata references do not point into mapped storage;
recorded frames copy only their requested bytes and retain that exact layout.

## Wire contracts

The [raw crate contract](../../crates/iracing-irsdk/src/lib.rs) owns representation
and intrinsic decoding limitations. Follow [Header](../../crates/iracing-irsdk/src/header.rs),
[VariableHeader](../../crates/iracing-irsdk/src/variable_header.rs), and
[VariableType](../../crates/iracing-irsdk/src/variable_type.rs) to definitions,
validation, and adjacent ABI/round-trip tests. Source geometry and live snapshot
obligations remain at their consuming boundaries below.

## Range and layout contracts

The [region module](../../crates/iracing-sdk/src/types/regions/mod.rs) and its
constructor docs own geometric postconditions and their limits. Start with
[ByteRegion](../../crates/iracing-sdk/src/types/regions/bytes.rs), then follow
the specialized constructor and adjacent tests for the property being audited.

[`IbtLayout`](../../crates/iracing-sdk/src/types/ibt/layout.rs) validates physical
geometry against a particular source length. It does not validate variable
semantics or the lifetime of externally mutated storage. `IbtFile` enforces source
bounds, complete reads, immutable snapshots, and telemetry layout construction;
session decoding/parsing remains separate from physical geometry.
