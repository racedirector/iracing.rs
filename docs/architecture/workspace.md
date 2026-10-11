# Workspace Structure

## Purpose

The workspace separates telemetry acquisition and decoding, derive-time adapter
generation, simulation lifecycle probing, native broadcast control, shared test
data, and end-user examples.

```text
workspace applications
  ├─ driver-inputs ───────────────► iracing-sdk
  └─ iracing-lifecycle-monitor ───► iracing-sdk
                                └─► iracing-simulation
iracing-sdk ── optional derive ───► iracing-sdk-derive
      │
      └───────────────────────────► iracing-irsdk
test-fixtures ────────────────────► iracing-irsdk
      └───────────────────────────► iracing-sdk
```

`iracing-sdk-derive` has a dev-dependency back on `iracing-sdk` for integration
coverage. That is a test-time relationship, not a runtime layering reversal.

## Crate responsibilities

### `iracing-irsdk`

Owns the native SDK contract that can be understood without knowing a data
source or runtime:

- fixed-layout `Header`, `DiskSubHeader`, `VariableBuffer`, and
  `VariableHeader` structures;
- intrinsic fixed-size wire decoding through the wire types' `try_from_bytes`
  methods;
- `VariableType`, constants, enum discriminants, flags, packed fields, and
  broadcast command values;
- optional schema metadata for those primitive definitions.

It does not navigate IBT files, access Windows shared memory, build telemetry
schemas or frames, parse session YAML, or orchestrate providers and streams.
`iracing-sdk` re-exports the complete crate through `iracing_sdk::irsdk` for
source compatibility.

### `iracing-sdk`

Owns source parsing, transport, schemas, and runtime behavior built on the wire
contract:

- `.ibt` parsing, navigation, and random/sequential file access.
- Telemetry variable schemas, frame packets, dynamic values, typed decoding,
  and integration of SDK enums/flags with `VarData`.
- Provider and connection abstractions for recorded and live telemetry.
- Background telemetry task orchestration and source-specific policies.
- Session YAML cleanup, typed deserialization, caching, and schema discovery.
- Windows shared-memory mapping and update waiting.
- CLI tools, schema generators, and crate-level examples.

Code that understands a source, schema, session document, or runtime belongs
here. Broadcast operation encoding and native dispatch belong to
[`iracing-broadcast-sdk`](../../crates/iracing-broadcast-sdk/src/lib.rs).

### `iracing-sdk-derive`

Owns the `IRacingTelemetryFrame` procedural macro. It converts field attributes
into a two-phase `FrameAdapter` implementation:

1. validate field names/types and build an extraction plan once;
2. adapt each frame using pre-resolved `FieldId` slots.

Generated code refers to the public `iracing_sdk` path. The SDK re-exports the
macro behind its default `derive` feature and provides a hidden tracing re-export
so consumers do not need tracing solely because of generated warning paths.

### `iracing-simulation`

Provides independent simulation-status probes. The
[crate Rustdoc](../../crates/iracing-simulation/src/lib.rs) owns its boundary and
links to HTTP and Windows process contracts.

### Workspace applications

`examples/driver-inputs` and `examples/iracing-lifecycle-monitor` are regular
workspace packages with `publish = false`. They should consume public crate APIs
as downstream programs do; do not make library internals public only to support
an example.

### CLI composition

The two CLI executables have separate composition roots and consumer-owned
capability traits. Implementation invariants and initialization policy live in
[`iracing-broadcast`'s application module](../../crates/iracing-broadcast-cli/src/application.rs)
and [`iracing-sdk`'s application module](../../crates/iracing-sdk-cli/src/application/mod.rs).
The [broadcast command crate](../../crates/iracing-broadcast-cli/src/lib.rs)
owns parsing and capability-based execution; follow the application modules for
the resources each executable supplies.

## Primary runtime paths

Recorded telemetry:

```text
.ibt bytes
  -> IbtReader
  -> IbtProvider
  -> Telemetry task
  -> IbtConnection / consumer
  -> FrameAdapter or DynamicFrame
```

Live telemetry:

```text
iRacing shared memory + update event
  -> WindowsConnection
  -> LiveProvider
  -> Telemetry task
  -> LiveConnection
  -> FrameAdapter or DynamicFrame
```

Broadcast command ownership is documented by the
[SDK](../../crates/iracing-broadcast-sdk/src/lib.rs) and
[CLI](../../crates/iracing-broadcast-cli/src/lib.rs). Raw wire values live in
[`iracing-irsdk`](../../crates/iracing-irsdk/src/broadcast.rs).

Simulation status: see the [source-owned probe contracts](../../crates/iracing-simulation/src/lib.rs).

## Placement rules

- Put intrinsic fixed-layout decoding and native SDK definitions in
  `iracing-irsdk`.
- Put source framing, file navigation, shared-memory transport, schema
  construction, session parsing, and runtime behavior in `iracing-sdk`.
- Put test-only fixture discovery in `iracing-sdk::test_utils` and fixture
  generation/verification in `scripts`.
- Gate the smallest OS-dependent implementation unit. Do not hide portable data
  models just because one transport is Windows-only.
