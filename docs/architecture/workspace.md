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

iracing-broadcast-grpc-service ───► iracing-sdk
iracing-sdk-ws (WebSocket facade; runtime composition lives in its binary)
iracing-sdk ── optional derive ───► iracing-sdk-derive
```

`iracing-sdk-derive` has a dev-dependency back on `iracing-sdk` for integration
coverage. That is a test-time relationship, not a runtime layering reversal.

## Crate responsibilities

### `iracing-sdk`

Owns the iRacing data and command vocabulary:

- `.ibt` wire-format parsing and random/sequential file access.
- Telemetry variable schemas, frame packets, dynamic values, typed decoding,
  bitfields, enums, and incident helpers.
- Provider and connection abstractions for recorded and live telemetry.
- Background telemetry task orchestration and source-specific policies.
- Session YAML cleanup, typed deserialization, caching, and schema discovery.
- Windows shared-memory mapping, update waiting, and broadcast message packing.
- CLI tools, schema generators, and crate-level examples.

Code that understands byte layout or Win32 iRacing transport belongs here.

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

### `iracing-broadcast-grpc-service`

Owns the network boundary for broadcast controls:

- the protobuf contract and generated tonic/prost bindings;
- gRPC request validation and response mapping;
- application use cases expressed against internal ports;
- adapters to SDK broadcast commands and telemetry/session observation;
- the Windows server binary with health and reflection services.

Raw Win32 message identifiers and packing remain in `iracing-sdk`. Generated
protobuf types remain portable; the real service composition is Windows-only.

### `iracing-sdk-ws`

Owns the portable Axum WebSocket boundary. The library exposes the facade
router, while listener binding and runtime startup belong to the crate's binary.
Protocol and telemetry behavior have not yet been added.

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

- Put byte decoding and variable type rules in `iracing-sdk`, not consumers.
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
