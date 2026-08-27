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

`reader::ibt::IbtRecording` parses and validates the fixed header, disk
sub-header, metadata regions, and fixed-size frame layout from immutable `.ibt`
data. It supports indexed frame snapshots without mutable position.
`reader::ibt::IbtReader` adds a cursor over that recording. Live
`WindowsConnection` interprets the related shared-memory header and rotating
buffers through the same positioned-access and header-region representations.

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

The current live read path is:

1. `WindowsConnection::get_new_data` selects the telemetry buffer with the
   highest tick count. It checks that buffer's tick before and after creating a
   borrowed byte slice and returns the slice when the two reads agree.
2. `LiveProvider::next_frame_impl` immediately copies that slice into an owned
   `Vec<u8>`. It then reads the shared-memory header again, selects the latest
   telemetry buffer again, and takes both the packet tick and
   `session_info_update` from that later header view.
3. The provider returns an owned `FramePacket` containing the frame bytes, tick,
   and observed session version. `Telemetry::read_task` passes the packet to
   `LiveSessionPolicy::observe` before publishing the frame.
4. When the packet's session version differs from the last observed version,
   the policy immediately calls `Provider::session_yaml`. For `LiveProvider`,
   this calls `WindowsConnection::session_info`, which reads the offset and
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

- `get_new_data` returns a borrowed slice; its tick consistency check finishes
  before `LiveProvider` copies the bytes. The provider also selects the latest
  buffer again for packet metadata, so the owned bytes and later tick/version
  reads are not one atomic snapshot.
- The `Provider::session_yaml` version argument is only a change trigger for
  `LiveProvider`; it is intentionally ignored rather than treated as a lookup
  key. `WindowsConnection::session_info` copies whichever YAML occupies the
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
