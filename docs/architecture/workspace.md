# Workspace Structure

## Purpose

The workspace separates telemetry acquisition and decoding, derive-time adapter
generation, simulation lifecycle probing, network broadcast control, shared test
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
- intrinsic fixed-size wire decoding through `WireType`;
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
- Windows shared-memory mapping, update waiting, and broadcast message packing.
- CLI tools, schema generators, and crate-level examples.

Code that understands a source, schema, session document, or runtime belongs
here. Win32 transport remains here even though the portable command values it
sends belong in `iracing-irsdk`.

### `iracing-sdk-derive`

Owns the `IRacingTelemetryFrame` procedural macro. It converts field attributes
into a two-phase `FrameAdapter` implementation:

1. validate field names/types and build an extraction plan once;
2. adapt each frame using pre-resolved `VariableInfo`.

Generated code refers to the public `iracing_sdk` path. The SDK re-exports the
macro behind its default `derive` feature and provides a hidden tracing re-export
so consumers do not need tracing solely because of generated warning paths.

### `iracing-simulation`

Owns simulation lifecycle checks that do not require telemetry:

- a portable HTTP probe for the local `get_sim_status` endpoint;
- a `SimStatusClient` injection seam;
- a raw-`TcpStream` default client with a deliberately small parser;
- Windows-only process enumeration for `iRacingSim64DX11.exe`.

It does not own shared-memory telemetry or broadcast commands.

### Workspace applications

`examples/driver-inputs` and `examples/iracing-lifecycle-monitor` are regular
workspace packages with `publish = false`. They should consume public crate APIs
as downstream programs do; do not make library internals public only to support
an example.

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

Broadcast RPC:

```text
gRPC request
  -> broadcast_service boundary mapping
  -> BroadcastUseCases
  -> command and observation ports
  -> iracing-sdk Win32 broadcast + live telemetry/session data
  -> observed snapshot or acknowledgement response
```

Simulation status:

```text
Simulation
  -> SimStatusClient
  -> GET /get_sim_status?object=simStatus
  -> 2xx + body contains running:1
  -> bool
```

## Placement rules

- Put intrinsic fixed-layout decoding and native SDK definitions in
  `iracing-irsdk`.
- Put source framing, file navigation, shared-memory transport, schema
  construction, session parsing, and runtime behavior in `iracing-sdk`.
- Put public protocol changes in `broadcast.proto` before generated/service code.
- Put orchestration that can be tested with fake ports in `broadcast_app`, not
  tonic handlers or Win32 adapters.
- Put external-boundary conversion in its adapter: protobuf conversion in
  `broadcast_service`, SDK/live conversion in `broadcast_iracing`.
- Put optional HTTP clients in simulation examples/dev-dependencies; preserve
  the raw standard-library default path.
- Put test-only fixture discovery in `iracing-sdk::test_utils` and fixture
  generation/verification in `scripts`.
- Gate the smallest OS-dependent implementation unit. Do not hide portable data
  models just because one transport is Windows-only.
