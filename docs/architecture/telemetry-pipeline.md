# Telemetry Pipeline

## Layers

The SDK separates acquisition, transport-neutral frames, delivery/session
policy, and consumer adaptation.

```text
source bytes
  -> reader or Windows connection
  -> Provider
  -> Telemetry::read_task
       ├─ DeliveryPolicy
       └─ SessionPolicy
  -> connection/subscriber
  -> FrameAdapter
```

This separation lets recorded files and live shared memory share frame and
adapter types without pretending that their timing and loss semantics are the
same.

## Wire and schema layer

`IbtReader` parses the fixed header, disk sub-header, variable headers, session
YAML region, and fixed-size frame records from `.ibt` data. `open` retains a
private read-only memory map plus decoded headers and layout, and reads one owned
frame on demand; `from_bytes` uses the same parser over an owned in-memory
cursor. `IbtLayout` is the canonical physical source description, exposing total
source length, main-header/disk-header/preamble regions, optional metadata bounds,
frame start/size/count, and O(1) indexed frame geometry. `IbtReader::layout()`
exposes the same validated description for inspection without additional reads
or cursor changes. Fixed regions are derived from the wire types; source length
comes from the EOF-delimited frame region. No duplicate coordinates are stored.
The layout describes byte geometry, not telemetry fields, replay state, or CLI
formatting. Source I/O uses checked conversions between `u64` seek offsets
and `usize` layout coordinates. Unlike the earlier `u64` file navigation, this
rejects sources larger than `usize::MAX` bytes: files of 4 GiB or more cannot be
opened on 32-bit targets. Supporting those files would require a separately
scoped wider layout API. File-backed readers require completed, immutable
recordings: no process may modify or truncate the file until the reader (or
owning provider/connection) is dropped. Use `from_bytes` with an owned copy when
that lifetime requirement cannot be met.

`frame(index)`, `session_info_snapshot()`, and `VariableHeadersProvider::variable_headers()` read
owned data from the source on each call. They may move its physical cursor;
there is no reader-owned logical cursor, schema, or session cache. Every indexed
read seeks to its validated region. Live `WindowsConnection` interprets the
related shared-memory header and rotating buffers.

`VariableSchema` maps names to `VariableInfo` and records the frame size. A
`VariableInfo` carries type, byte offset, element count, time-count marker,
units, and description. Schema construction is the boundary at which ranges
should be validated.

Telemetry is little-endian. `VarData::from_bytes` and `TelemetryValue::decode`
are the authoritative decoding paths; consumers should not reproduce byte
slicing or discriminant handling.

### Windows source ownership and validation

An internal `LiveSource` owns the mapping handle, mapped view, and event.
`VirtualQuery` bounds access to the mapped region; connection activation rejects
regions shorter than the fixed header before unchecked scalar accessors become
available. Scalar words use aligned volatile reads, while bulk copies use a
native routine into owned storage. Frame and metadata ranges are checked before
copying, including negative geometry and overflow. Mutable header geometry is
validated on each acquisition rather than trusted from connection setup.

An asynchronous wait retains the event through an `Arc` in its blocking worker.
Canceling the awaiting future or dropping the connection does not close the event
while that worker is waiting. Requested timeouts are capped below Windows'
`INFINITE` sentinel; cancellation does not interrupt the underlying wait.

Windows unit tests use private mappings and events to exercise acquisition,
malformed regions, cancellation, and handle release without iRacing. Simulator
publication timing and performance require an active simulator and remain manual
verification; the ignored `iracing_required` tests provide connection smoke tests.

## `FramePacket`

Providers return `FramePacket`, the common data unit:

- owned, reference-counted frame bytes (`Arc<[u8]>`);
- monotonic source tick;
- session version;
- shared `VariableSchema`.

The packet can decode a named value directly. `DynamicFrame` wraps the same
bytes and schema for exploratory name-based lookup. Hot paths should implement
`FrameAdapter` so lookup and type checking happen once.

## Providers

`Provider` is an async, owned-source abstraction with three responsibilities:

- return the next frame or permanent EOF;
- return session YAML when available;
- report the source tick rate.

`IbtProvider::from_reader` validates the exact variable-header snapshot against
the layout's frame size and owns the resulting shared schema. Construction
starts its sequential cursor at frame zero, even after indexed reader operations.
Frames without variable metadata are rejected; zero-frame recordings may have
an empty schema. Successful reads advance one index, failed reads retain the
index for retry, and the layout's frame count determines permanent EOF. Packets
use the zero-based index cast to `u32` as their synthetic tick and retain the
header's session update counter. Tick rate comes from the header with a 60 Hz
fallback for nonpositive values. Session YAML is read from a fresh snapshot,
decoded and sanitized by the provider. Reads complete as fast as the file can be
decoded; the provider has no seek/time helper API.

`LiveProvider` is Windows-only. It builds a schema from shared-memory metadata,
waits cooperatively for updates, returns the newest owned frame snapshot, and
polls until iRacing connects or its configured no-connection limit is reached.
The provider itself supplies live pacing.

### Live session update acquisition

iRacing exposes live session information differently from telemetry frames.
Telemetry has rotating buffers identified by tick count, while the header
describes one current session YAML region with a `session_info_update` counter.
The SDK does not expose a history of session YAML regions: a consumer must copy
the current region before another update replaces it if it needs every
observable intermediate state.

The current live read path is:

1. `WindowsConnection::get_new_data` selects the published `current_buffer`,
   falling back to buffer zero for invalid indices/counts. The first tick or an
   older tick establishes a baseline without delivery; equal ticks yield no data.
   It copies into reusable connection-owned storage, accepting the frame only
   when the completed tick before copying equals the begin tick afterward,
   with at most two attempts using the same descriptor.
2. `LiveProvider::next_frame_impl` copies the accepted bytes into its owned
   packet. It reads a separate owned header snapshot for the session version
   and uses the connection's accepted `last_tick_count` for the packet tick.
3. The provider returns an owned `FramePacket` containing the frame bytes, tick,
   and observed session version. `Telemetry::read_task` passes the packet to
   `LiveSessionPolicy::observe` before publishing the frame.
4. When the packet's session version differs from the last observed version,
   the policy immediately calls `Provider::session_yaml`. For `LiveProvider`,
   this calls `SessionInformationBytesProvider::session_info_snapshot`, which reads the offset and
   length from the current header and copies/extracts the one current YAML
   region into an owned `String`.
5. `LiveProvider` performs iRacing YAML preprocessing on that owned string.
   Typed `SessionInfo` deserialization is then dispatched away from the frame
   task.

This ordering intentionally keeps shared-memory acquisition ahead of typed
deserialization. Deferring the YAML copy itself to a background parser would
leave only a version number queued while the corresponding mutable region could
already have been overwritten. Copying and cleaning the string does briefly
hold the frame task, but once the policy owns the string, parsing can proceed
without delaying acquisition of the next telemetry frame and its possible next
session version.

The current implementation has several consistency limits that matter when
reasoning about session ordering:

- `get_new_data` returns a slice of connection-owned bytes whose copy was
  checked against synchronization words. Packet session metadata comes from a
  later header snapshot; the bytes and session version are not one atomic
  snapshot. Header snapshots themselves can span publication instants.
- The `Provider::session_yaml` version argument is only a change trigger for
  `LiveProvider`; it is intentionally ignored rather than treated as a lookup
  key. `SessionInformationBytesProvider::session_info_snapshot` copies whichever YAML occupies the
  single current session region at that moment. That copy does not compare
  `session_info_update` before and after reading the region.
- The data-valid event can signal a session-only change, but
  `next_frame_impl` returns only after `get_new_data` finds a new telemetry
  tick. Session version discovery is therefore associated with the next frame
  the provider accepts, not with an independently emitted session event.
- If iRacing replaces the session region more than once before this process
  observes and copies it, the overwritten intermediate contents cannot be
  reconstructed by downstream ordering logic.

These limits define the strongest useful ordering contract: preserve every
session snapshot that was successfully observed and copied, associate it with
the frame version/tick that caused its discovery, and never reorder or silently
coalesce those owned snapshots afterward. They do not establish that every
session version produced by iRacing can always be recovered.

Once an owned YAML snapshot has been captured, parsing does not need to be
concurrent. A single background FIFO parser can keep typed deserialization off
the frame task while naturally preserving observation order. Any observer-facing
event stream must preserve the same FIFO property; a latest-value channel may
remain useful for `current_session`, but it cannot by itself represent a
lossless sequence of session changes.

## Telemetry task

`Telemetry::read_task` owns a provider. It initializes the session policy, then
repeats:

1. acquire one delivery permit;
2. call `Provider::next_frame` with cancellation selection;
3. let the session policy observe a successful packet;
4. deliver the packet through the delivery policy.

Provider errors use exponential backoff and stop after ten consecutive errors.
Dropping a high-level connection cancels its task through a
`CancellationToken`. The task finalizes its session policy exactly once on every
exit, including cancellation, provider EOF, terminal errors, and dropped frame
receivers.

The internal `TelemetryBuilder` makes delivery and session policies independent.
Its defaults are `LatestDelivery` and `LiveSessionPolicy`.

## Delivery policies

`LatestDelivery` stores `Option<Arc<FramePacket>>` in a Tokio watch channel.
Each frame replaces the previous snapshot. This is correct for live state:
consumers care about the newest value and may intentionally miss intermediate
ticks.

`OnDemandDelivery` uses an mpsc request queue and one-shot responses. One demand
authorizes exactly one provider read. `Telemetry::spawn_ibt` selects this policy
and returns its request handle to `IbtConnection`.

`IbtConnection` places a coordinated watch bridge above that request handle.
The connection starts explicitly, maintains one shared IBT cursor, and publishes
one retained frame to every active subscription. A subscription acknowledges its
current frame when it is polled for the next item. The bridge sends another
demand only after every active subscription has acknowledged the retained frame.
Dropping the final subscription parks the cursor without closing the connection;
a later subscriber receives the retained frame and can resume replay.

## Session policies

`LiveSessionPolicy` watches packet session versions. On a changed version it
fetches and owns the current YAML immediately, then submits the snapshot to a
single background FIFO parser so typed YAML deserialization does not block the
frame loop. The current semantics are:

- a version is marked observed even if fetch or parse fails;
- repeated adjacent frames with the same version do not refetch YAML;
- owned snapshots are parsed and published one at a time in observation order;
- a parse failure is logged and does not prevent a later queued snapshot from
  being parsed;
- `end` closes the task queue, drains every queued parse, and only then
  publishes `None`.

Live session publication still uses a watch channel. FIFO parsing determines
send order, but the channel retains only the latest value and can coalesce
updates that an observer does not receive promptly. Lossless observer delivery
is a separate policy concern.

`IbtSessionPolicy` fetches the file's single immutable YAML document once during
initialization, parses inline, and publishes before frames. It does not retry a
missing, failed, or malformed session, and it retains successful metadata after
EOF.

These behaviors are explicit policies because live-changing state and immutable
recording metadata have different lifecycle requirements.

## Connections

`IbtConnection` and `LiveConnection` are convenience facades that:

- build or accept a provider;
- spawn the telemetry task;
- expose typed frame subscriptions and session update streams;
- retain current frame/session snapshots;
- cancel background work on drop.

`LiveConnection` normalizes `UpdateRate` against source frequency and applies
latest-wins throttling. `IbtConnection` does not accept an update rate: recorded
delivery is paced by its coordinated subscriber acknowledgement barrier.

`LiveConnection` has a portable non-Windows stub whose builder returns an
unsupported-platform error. The actual fields and subscription methods exist
only on Windows.

Both connection subscription methods currently panic if adapter schema
validation fails. Treat this as an existing public-API limitation, not a pattern
to copy into new fallible boundaries.

## Adapter pattern

`FrameAdapter` has a deliberate two-phase contract:

1. `validate_schema` maps requested fields to `VariableInfo` and returns
   `AdapterValidation`;
2. `adapt` decodes each `FramePacket` using that precomputed plan.

`FieldExtraction` represents required, optional, defaulted, calculated, and
skipped strategies. The derive crate generates this plan from
`IRacingTelemetryFrame` attributes.

Invariants:

- required schema mismatches fail during validation;
- per-frame adaptation should avoid schema hash-map lookup;
- decoding goes through `VarData`;
- `DynamicFrame` is for flexibility, not the default hot-path design.

## Rate limiting

For live telemetry, `UpdateRate::Native` forwards source cadence and
`UpdateRate::Max(hz)` applies the custom `ThrottleExt` stream after frame
delivery. Throttling and delivery loss are separate concerns: a live source can
already have dropped frames before a subscriber-level throttle runs.
Coordinated IBT subscriptions do not use this throttle.
