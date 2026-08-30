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

`reader::ibt::IbtReader` directly owns immutable `.ibt` bytes, the parsed common
and disk headers, checked metadata regions, fixed frame geometry, and one frame
cursor. Construction validates the complete layout once; indexed and sequential
reads then perform only frame-index checks and owned slice copies.

`reader::live::LiveReader` is the corresponding concrete boundary for one live
mapping generation. Construction validates the static mapping extent, frame
regions, descriptor control offsets, and variable-header region. Normal reads
retain only dynamic status/index/tick checks and the SDK consistency protocol.
`WindowsConnection` owns the Win32 handles and reconstructs the reader whenever
a mapping is opened or replaced; it does not duplicate pointer arithmetic or
buffer-selection policy.

`VariableSchema` maps names to `VariableInfo` and records the frame size. A
`VariableInfo` carries type, byte offset, element count, time-count marker,
units, and description. Schema construction is the boundary at which ranges
should be validated.

Telemetry is little-endian. `VarData::from_bytes` and `TelemetryValue::decode`
are the authoritative decoding paths; consumers should not reproduce byte
slicing or discriminant handling.

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

`IbtProvider` wraps `IbtReader`, converts owned variable-header snapshots into
`VariableSchema`, cleans the owned session YAML snapshot, and assigns replay
ticks when it builds `FramePacket` values. It preserves the reader cursor and
exposes seek/time metadata; reads complete as fast as the file can be copied.

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

The live read path is:

1. `WindowsConnection::next_frame` creates an ephemeral `MappedView` for its
   still-owned mapping and delegates to the generation-bound `LiveReader`.
2. `LiveReader` reads connection status, validates the dynamic current-buffer
   index, and reads that descriptor's completed tick with a volatile load.
   Unchanged or reset ticks return before allocation.
3. For a new tick, the reader executes the SDK 1.20 sequence directly: read
   barrier, one owned copy of the prevalidated frame region, read barrier, then
   a volatile `tickCountBegin` read from the same descriptor. Equality accepts
   the frame; contention retries once without advancing the baseline.
4. The accepted `LiveFrameSnapshot` owns the bytes, descriptor tick, and session
   update observed by that attempt. `LiveProvider` converts those values into a
   `FramePacket` without rereading the header or selecting a buffer again.
5. When the packet version changes, session acquisition reads update/offset/
   length, checks and copies the dynamic YAML region, applies the ordering
   boundary, and accepts only if the complete proof is unchanged afterward.
   `LiveProvider` then performs YAML preprocessing and dispatches typed parsing.

This ordering intentionally keeps shared-memory acquisition ahead of typed
deserialization. Deferring the YAML copy itself to a background parser would
leave only a version number queued while the corresponding mutable region could
already have been overwritten. Copying and cleaning the string does briefly
hold the frame task, but once the policy owns the string, parsing can proceed
without delaying acquisition of the next telemetry frame and its possible next
session version.

The remaining consistency limits are producer constraints rather than split
reader ownership:

- The `Provider::session_yaml` version argument is a change trigger rather than
  a historical lookup key. Shared memory exposes only the one current YAML
  region, so acquisition returns the actual stable version copied at that time.
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
