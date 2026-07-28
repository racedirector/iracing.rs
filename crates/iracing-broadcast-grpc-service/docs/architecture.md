# iRacing Broadcast gRPC Service Architecture

This crate exposes iRacing broadcast controls over gRPC. It separates the
protobuf boundary, command orchestration, and SDK/live adapters so request
validation and simulator-specific I/O do not become one monolithic tonic
service.

The service has two kinds of operation:

- state-bearing RPCs snapshot simulator data, send a command, and wait for a
  matching telemetry change;
- acknowledgement RPCs validate and synchronously send a command without
  claiming that simulator state was observed.

## Goals

- Keep `proto/broadcast.proto` as the public network contract.
- Keep raw Win32 broadcast packing in `iracing-sdk`.
- Validate untrusted protobuf input at the gRPC boundary.
- Express command sequencing against internal ports that can be faked in tests.
- Return observed simulator state where the SDK exposes enough telemetry/session
  data.
- Keep generated protobuf and tonic client/server types cross-platform.
- Keep real live composition Windows-only.

## Non-goals

- The server does not provide TLS, authentication, authorization, rate limiting,
  or tenant isolation.
- A successful acknowledgement does not prove that iRacing applied a command.
- State observation is not transactional with command sending.
- The crate does not replace the SDK's general telemetry streaming APIs.
- There is no separate ergonomic client wrapper; consumers use
  `RawBroadcastClient` or generated tonic types directly.

## Source layout

```text
crates/iracing-broadcast-grpc-service/
  build.rs
  proto/
    broadcast.proto
  src/
    lib.rs
    broadcast_app/
      error.rs
      model.rs
      ports.rs
      use_cases.rs
    broadcast_iracing/
      command_sender.rs
      observation.rs
    broadcast_service/
      builder.rs
      command.rs
      error.rs
      request.rs
      response.rs
      mod.rs
    telemetry_observer.rs
    bin/
      server.rs
  tests/
    grpc_transport.rs
```

`build.rs` uses `tonic-prost-build` and `protoc-bin-vendored`. Generated code
lives in `OUT_DIR`; never edit it directly. The embedded descriptor set is
exported as `FILE_DESCRIPTOR_SET` for reflection.

## Platform split

`lib.rs` always exposes:

- generated protobuf messages and enums;
- `RawBroadcastClient`;
- the generated `Broadcast` trait and `BroadcastServer`;
- `FILE_DESCRIPTOR_SET`.

The application, observation, service, and concrete builder modules are
currently compiled only on Windows. `BroadcastService` and
`BroadcastServiceBuilder` therefore exist only on Windows.

The server binary has a non-Windows entry point that returns a clear
unsupported-platform error.

## Layering

```text
gRPC/HTTP2
  -> broadcast_service
       boundary validation
       protobuf <-> application/SDK conversion
       tonic Status mapping
  -> broadcast_app
       models and expectations
       command/state ports
       use-case sequencing
  -> broadcast_iracing
       SDK command sender
       telemetry/session-backed port implementation
  -> iracing-sdk
       Win32 broadcast channel
       live telemetry frames and session YAML
  -> iRacing
```

### gRPC adapter: `broadcast_service`

`BroadcastService` is intentionally thin. Each RPC:

1. records the remote address and selected request fields in a tracing span;
2. destructures the protobuf request;
3. validates and narrows external values;
4. maps enums/messages to SDK command data;
5. calls one `BroadcastUseCases` method;
6. maps the application result to protobuf or tonic `Status`.

Keep protobuf-specific concerns here. Do not put session lookup or frame polling
inside tonic handlers.

`request.rs` owns boundary primitives such as:

- optional/required `u16`, `i16`, `u32`, string, enum, and float conversion;
- narrowing range checks;
- finite-float checks;
- whole-number float-to-`u16` conversion used by pit commands.

`command.rs` maps protobuf command enums and messages to SDK types.
`response.rs` maps observed application snapshots back to protobuf.

### Application layer: `broadcast_app`

`model.rs` defines transport-independent snapshots and expectations for camera,
replay, pit, telemetry logging, and force feedback.

`ports.rs` defines:

- `BroadcastCommandPort`;
- `CameraStatePort`;
- `ReplayStatePort`;
- `PitStatePort`;
- `TelemetryStatePort`;
- `ForceFeedbackStatePort`.

`BroadcastUseCases` depends only on these ports. It owns snapshot/send/wait
sequencing, optional-field fallback, observation timeout use, and ordered
multi-command behavior.

`DisabledObservationPort` implements all state ports with
`ObservationDisabled`. This supports an ack-only service configuration without
pretending that state-bearing RPCs can work.

### iRacing adapters: `broadcast_iracing`

`IracingBroadcastCommandSender` wraps `iracing_sdk::Broadcast` and implements
`BroadcastCommandPort`. It delegates message packing and the Win32 send call to
the SDK.

`IracingObservation` implements all state ports over one shared live provider.
It:

- validates telemetry adapter capabilities against the provider schema;
- serializes provider reads behind a Tokio mutex;
- uses typed telemetry adapters for snapshots and change detection;
- parses and caches session YAML by version;
- derives camera groups and car-number/index mappings from session data;
- converts invalid observed values to `FailedPrecondition`.

### Telemetry observer

`TelemetryObserver` is generic over the provider contract expected by the
service. It holds a shared provider and schema. `snapshot_observed` reads one
frame and adapts it. `wait_for_change_matching_observed` reads until:

- the adapted value changes and matches the supplied predicate;
- the timeout expires;
- the source ends;
- the provider returns an SDK error.

The provider mutex prevents concurrent RPCs from reading the same live provider
simultaneously. It does not globally serialize command sends.

## Service construction

`BroadcastServiceBuilder` defaults to:

- opening a new SDK broadcast client;
- opening live observation;
- a two-second observation timeout.

Configuration methods:

- `with_client` injects an existing SDK broadcast client;
- `with_live_provider` injects an already-created live provider;
- `with_observation_timeout` changes only state-wait duration;
- `without_observation` avoids live provider creation and installs disabled
  state ports.

With observation disabled, acknowledgement RPCs can still send. State-bearing
RPCs fail with `FAILED_PRECONDITION`.

## Request and response semantics

### State-bearing RPCs

These operations use observation:

| Group | Operations | Returned state |
| --- | --- | --- |
| Camera | list cameras, switch by position/number, set state | Camera/session-derived catalog or observed camera selection/state |
| Replay | set speed, set play position, search | Observed replay speed or position |
| Pit | unary and streamed pit commands | Observed pit-service snapshot |
| Telemetry | start/restart/stop logging | Observed logging flags |
| Force feedback | set max force | Observed max-force value |

Optional fields on camera switch/state and replay speed mean "use the current
observed value." They are not protobuf defaults. This makes a partial request an
explicit read-modify-command operation.

Typical lifecycle:

```text
snapshot previous value
  -> resolve omitted fields
  -> send BroadcastCommand
  -> wait for changed matching telemetry
  -> return observed snapshot
```

Replay position/search and pit commands currently wait for any relevant
snapshot change rather than proving every command parameter directly.

### Acknowledgement RPCs

Replay state, texture reload, chat, replay search-by-session-time, and video
capture return empty acknowledgement responses after validation and synchronous
SDK send success. They are not simulator-state confirmation.

### Pit command stream

The tonic adapter reads and validates the complete client stream before invoking
the use case. It rejects:

- an empty stream with `INVALID_ARGUMENT`;
- more than 1000 commands with `RESOURCE_EXHAUSTED`.

The use case snapshots pit state, sends buffered commands sequentially, then
waits for a change. A later send or the final observation can fail after earlier
commands were sent. There is no rollback.

## Validation rules

- Reject absent required values.
- Reject protobuf `UNKNOWN` enum values and invalid numeric discriminants.
- Check every `u32 -> u16` and `i32 -> i16` conversion.
- Require finite floats.
- Require pit values represented as floats to be whole numbers that fit `u16`
  where the SDK command is integer-valued.
- Keep chat macros in the supported range.
- Reject empty strings where the operation requires content.
- Validate the whole pit stream before sending any item.

Bad external input maps to `INVALID_ARGUMENT` except for the explicit stream
size limit, which maps to `RESOURCE_EXHAUSTED`.

## Error mapping

Application errors map as follows:

| Error | gRPC status |
| --- | --- |
| Observation timeout | `DEADLINE_EXCEEDED` |
| Observation source ended | `UNAVAILABLE` |
| Observation disabled | `FAILED_PRECONDITION` |
| Telemetry capability unavailable | `FAILED_PRECONDITION` |
| Invalid observed/current state | `FAILED_PRECONDITION` |

SDK connection, Windows API, retryable buffer, and other retryable errors map to
`UNAVAILABLE`. Unsupported platform maps to `FAILED_PRECONDITION`. Unexpected
non-retryable SDK errors map to `INTERNAL`.

SDK failures are logged with retryability. Error text must remain useful to an
operator without exposing unrelated process details or secrets.

## Ordering, concurrency, and retries

- Tonic may execute independent RPCs concurrently.
- Provider reads used for observation are serialized by the observation mutex.
- SDK command sends are not globally queued.
- Commands inside one pit stream are sent in stream order.
- The simulator's handling order is outside this crate's control.
- Retrying a command RPC may send the command more than once.
- A timeout happens after the command was sent; it does not undo the command.

If strict global command ordering becomes necessary, add an explicit application
port/queue and document its backpressure and latency semantics.

## Server runtime

On Windows, `src/bin/server.rs`:

1. initializes tracing from `RUST_LOG` or its built-in filter;
2. reads `BROADCAST_ADDR`, defaulting to `[::]:50051`;
3. creates `LiveProvider` and injects it into the service;
4. builds health and reflection v1/v1alpha services;
5. creates an IPv4 or dual-stack IPv6 socket;
6. serves reflection, health, and broadcast services.

The default is an all-interfaces dual-stack bind, not loopback-only. This can
expose simulator controls to the network. The crate has no built-in security;
use firewall/network policy, TLS termination, and authentication before exposing
it outside a trusted host/network.

## Extending the service

For a new command:

1. Change `proto/broadcast.proto`.
2. Rebuild and use generated types; never edit `OUT_DIR`.
3. Add boundary validation and protobuf conversion.
4. Add or extend application models/ports if state is observed.
5. Implement orchestration in `BroadcastUseCases`.
6. Implement SDK/live behavior in `broadcast_iracing`.
7. Map the response and error semantics.
8. Cover invalid input, omitted-field behavior, send failure, observation
   timeout/source end, and response mapping.
9. Update this document and the crate `AGENTS.md`.

For a new observed telemetry shape, implement a small `FrameAdapter`, validate
it once at observation construction, and keep numeric/semantic conversion at the
iRacing adapter boundary.

## Testing strategy

- Test use cases with fake command and state ports.
- Test request/command/response converters at the gRPC boundary.
- Test telemetry observation with fake providers and minimal schemas.
- Test transport behavior over a real tonic listener when the platform permits.
- Keep protocol-generation and raw-client types compiling cross-platform.
- Run the crate check on Windows because most implementation modules are
  Windows-gated.

Relevant commands:

```text
cargo test -p iracing-broadcast-grpc-service
cargo check -p iracing-broadcast-grpc-service --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings
```

## Current integration inconsistencies

The Windows implementation currently does not compile against the SDK facade:

- observation code imports and implements an SDK trait named `SendProvider`,
  while the SDK defines `provider::Provider` and has no `SendProvider`;
- builder and observation code import `LiveProvider` from the SDK root, while
  its current path is `iracing_sdk::providers::live::LiveProvider`;
- observation code imports `SessionInfo` and `SessionInfoParser` from the SDK
  root, while their current path is `iracing_sdk::schema`.

Do not work around these mismatches in protobuf or tonic code. Align the SDK
provider contract and adapter imports, restore the Windows build, then update
this section.
