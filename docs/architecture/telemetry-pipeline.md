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
YAML region, and fixed-size frame records from `.ibt` data. Live
`WindowsConnection` interprets the related shared-memory header and rotating
buffers.

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

`IbtProvider` wraps `IbtReader`, preserves its cursor, and exposes seek/time
metadata. Its reads complete as fast as the file can be decoded.

`LiveProvider` is Windows-only. It builds a schema from shared-memory metadata,
waits cooperatively for updates, returns the newest owned frame snapshot, and
polls until iRacing connects or its configured no-connection limit is reached.
The provider itself supplies live pacing.

## Telemetry task

`Telemetry::read_task` owns a provider. It initializes the session policy, then
repeats:

1. acquire one delivery permit;
2. call `Provider::next_frame` with cancellation selection;
3. let the session policy observe a successful packet;
4. deliver the packet through the delivery policy.

Provider errors use exponential backoff and stop after ten consecutive errors.
Dropping a high-level connection cancels its task through a
`CancellationToken`.

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

`LiveSessionPolicy` watches packet session versions. On a new version it fetches
YAML once and parses it in a detached task so the frame loop is not blocked. The
current semantics are important:

- a version is marked observed even if fetch or parse fails;
- independently spawned parses can publish out of order;
- normal end publishes `None`.

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
