# iRacing Broadcast gRPC Service Architecture

This document describes the architecture of the `iracing-broadcast-grpc-service`
crate. The service exposes iRacing broadcast controls over gRPC while keeping
the Win32-specific broadcast transport isolated behind the lower-level
`iracing-sdk` crate.

The current implementation is a command gateway plus a live observation layer.
Command RPCs validate a protobuf request, convert it into typed SDK command
data, send it through the Windows broadcast channel, and either acknowledge the
send or return observed simulator state. Query RPCs read current state from live
telemetry and session data without sending broadcast commands. Subscription RPCs
stream those same current-state projections to clients as observed values
change.

## Goals

- Provide a network-accessible gRPC surface for iRacing broadcast commands.
- Preserve the typed command model from `iracing-sdk` instead of duplicating raw
  iRacing message packing in the service crate.
- Keep the server implementation Windows-only because iRacing broadcast
  messages use Win32 APIs.
- Keep generated protobuf types and tonic client/server types available to
  downstream consumers.
- Validate all external gRPC input at the service boundary before it reaches the
  SDK broadcast layer.

## Non-goals

- This crate does not stream raw live telemetry or session YAML to gRPC
  clients. Server-streaming RPCs expose only curated current-state projections
  that already have unary query responses.
- This crate does not currently prove that iRacing applied a command. It proves
  either that the command request was valid and the Win32 broadcast send path
  did not report an error, or that a supported observed state changed before the
  configured timeout.
- This crate does not implement authentication, authorization, TLS, rate
  limiting, or multi-tenant isolation. Those concerns belong in the deployment
  wrapper until the crate grows first-class support.

## Crate Layout

```text
crates/iracing-broadcast-grpc-service/
  Cargo.toml
  build.rs
  proto/
    broadcast.proto
  src/
    lib.rs
    broadcast_app/
    broadcast_iracing/
    broadcast_service/
    telemetry_observer.rs
    bin/
      server.rs
  docs/
    architecture.md
```

### `proto/broadcast.proto`

`broadcast.proto` is the public protocol contract. It defines:

- Request and response messages for camera, replay, texture, chat, pit,
  telemetry, force feedback, and video capture commands.
- Service enums with explicit `UNKNOWN = 0` values.
- The `Broadcast` service, unary command/query RPCs, the client-streaming
  `PitCommandStream` RPC, and server-streaming current-state subscription RPCs.

The protobuf uses `optional` primitive fields for inputs where absence is
meaningful. This lets the service distinguish "field omitted" from a primitive
default such as `0` or `false`, which is important for actionable
`INVALID_ARGUMENT` errors.

### `build.rs`

`build.rs` compiles `proto/broadcast.proto` at build time with
`tonic-prost-build` and a vendored `protoc` binary from `protoc-bin-vendored`.
This avoids requiring developers or CI runners to install `protoc` separately.

The generated code is included from `src/lib.rs` with:

```rust
tonic::include_proto!("iracing.broadcast");
```

### `src/lib.rs`

`lib.rs` is the crate facade. It:

- Includes the generated protobuf module as `broadcast`.
- Re-exports generated message, enum, client, service trait, and server types.
- Re-exports `BroadcastService` only on Windows.

The server implementation is behind `#[cfg(windows)]` because it depends on
`iracing_sdk::Broadcast`, which sends Win32 broadcast window messages.

### `src/broadcast_service/`

`broadcast_service/` contains the server-side tonic adapter and is compiled
only on Windows. The central type is `BroadcastService`.

Responsibilities:

- Implement the generated tonic `Broadcast` trait.
- Validate protobuf requests and return gRPC `Status` errors for bad input.
- Convert protobuf enums and messages into SDK command types.
- Delegate command/query behavior to `BroadcastUseCases`.
- Map domain and SDK errors into gRPC status codes.

The service is built through `BroadcastService::builder()`. Tests and future
composition layers can inject an existing SDK broadcast client with
`BroadcastServiceBuilder::with_client`.

### `src/broadcast_app/`

`broadcast_app/` contains the domain-facing use cases, model snapshots, ports,
and domain error types. It owns command orchestration rules such as "snapshot
current state, send command, wait for expected observed state".

Responsibilities:

- Keep gRPC/protobuf conversion out of core command orchestration.
- Define observation ports for camera, replay, pit, telemetry, force-feedback,
  and video-capture state.
- Define command ports for sending typed `iracing_sdk::BroadcastCommand` values.

### `src/broadcast_iracing/` and `src/telemetry_observer.rs`

`broadcast_iracing/` adapts the use-case ports to `iracing-sdk`. It owns the
live broadcast sender and the live telemetry/session observation adapter.
`telemetry_observer.rs` provides reusable frame snapshot and wait-for-change
helpers around typed telemetry frame adapters.

### `src/bin/server.rs`

`server.rs` is the executable entry point for the server binary
`iracing-broadcast-server`.

On Windows it:

1. Initializes `tracing_subscriber`.
2. Reads `BROADCAST_ADDR`, defaulting to `[::1]:50051`.
3. Creates a `BroadcastService`.
4. Starts a tonic transport server and registers `BroadcastServer`.

On non-Windows platforms the binary exits with an unsupported-platform error.

## Runtime Topology

```text
Remote client process
  |
  | gRPC over HTTP/2
  v
iracing-broadcast-server on Windows
  |
  | tonic generated server dispatch
  v
BroadcastService RPC method
  |
  | validate protobuf request
  | convert protobuf types to iracing-sdk types
  v
iracing_sdk::Broadcast::send_message
  |
  | RegisterWindowMessageW("IRSDK_BROADCASTMSG")
  | SendNotifyMessageW(HWND_BROADCAST, ...)
  v
iRacing simulator process
```

The gRPC service does not talk to iRacing directly. It delegates the final
transport hop to `iracing-sdk`, which owns the Win32 message contract and packs
the iRacing broadcast message id plus three command variables into
`WPARAM`/`LPARAM`.

## Request Lifecycle

Every server RPC follows the same high-level lifecycle:

1. Tonic receives the protobuf request and dispatches it to `BroadcastService`.
2. The RPC method destructures the generated request message.
3. Helper functions validate required fields, numeric ranges, finite floats,
   and enum values.
4. The method maps protobuf values into `iracing_sdk` command types.
5. Command RPCs delegate to `BroadcastUseCases`, which send through the SDK
   command port and, when observation is enabled, wait for the matching live
   telemetry state.
6. Query RPCs delegate to `BroadcastUseCases` snapshot methods backed by live
   telemetry/session observation ports.
7. Domain and SDK errors are mapped to gRPC status codes.
8. The method returns an acknowledgement, observed state, or an empty response
   depending on the RPC contract.

Example path for `ReplaySetPlayPosition`:

```text
ReplaySetPlayPositionRequest
  -> require non-UNKNOWN mode
  -> require frame
  -> map ReplayPositionMode to iracing_sdk::ReplayPositionMode
  -> BroadcastCommand::ReplaySetPlayPosition(mode, frame)
  -> BroadcastUseCases::replay_set_play_position(mode, frame)
  -> wait for replay position telemetry to change
  -> ReplaySetPlayPositionResponse { observed frame }
```

## Protocol Surface

The protobuf service currently includes these RPC groups:

| Group | RPCs | Current behavior |
| --- | --- | --- |
| Camera | `GetAvailableCameras`, `CurrentCameraPosition`, `CurrentCameraState`, `CameraSwitchPosition`, `CameraSwitchNumber`, `CameraSetState` | Camera queries read live telemetry/session data. Switch/set commands are sent and return observed camera state when observation is enabled. |
| Replay | `CurrentReplayPlaySpeed`, `CurrentReplayPosition`, `ReplaySetPlaySpeed`, `ReplaySetPlayPosition`, `ReplaySearch`, `ReplaySetState`, `ReplaySearchSessionTime` | Replay queries read live telemetry. Replay speed, position, and search commands return observed replay state when observation is enabled; replay-state and session-time commands are ack-only. |
| Textures | `ReloadTextures` | Sends either reload-all or reload-by-car-index. |
| Chat | `ChatCommand` | Sends chat mode commands or validated chat macros. |
| Pit service | `CurrentPitService`, `PitCommand`, `PitCommandStream` | Pit queries read live telemetry. Commands send one or more pit-service updates and return observed pit-service state when observation is enabled. |
| Telemetry logging | `CurrentTelemetryState`, `TelemetryCommand` | Telemetry queries read live logging state. Commands send disk telemetry logging commands and return observed logging state when observation is enabled. |
| Force feedback | `CurrentForceFeedback`, `ForceFeedbackCommand` | Force-feedback queries read live max-force telemetry. Commands send max-force updates and return observed max-force state when observation is enabled. |
| Video capture | `CurrentVideoCapture`, `VideoCapture` | Video capture queries read live capture telemetry. Commands send video capture requests and remain ack-only. |

Server-to-client subscription RPCs mirror the unary `Current*` state reads:
`SubscribeCurrentCameraPosition`, `SubscribeCurrentCameraState`,
`SubscribeCurrentReplayPlaySpeed`, `SubscribeCurrentReplayPosition`,
`SubscribeCurrentPitService`, `SubscribeCurrentTelemetryState`,
`SubscribeCurrentForceFeedback`, and `SubscribeCurrentVideoCapture`. Each stream
emits the current value first, then emits subsequent values only when the
response value changes. These streams let UI clients stay synchronized when a
state changes outside the gRPC command path, such as when an operator controls
the camera directly in the simulator.

`PitCommandStream` is client-streaming because multiple pit-service selections
are often applied together. The server validates and sends each streamed command
in order. If a later command is invalid or the SDK send path fails, the stream
returns an error after any earlier commands have already been sent. There is no
transactional rollback in the iRacing broadcast channel.

## Type Mapping

The service intentionally keeps protobuf types separate from SDK types.

Inbound mapping happens in `broadcast_service/`:

- Protobuf enums reject `UNKNOWN` before conversion.
- Protobuf integer fields are range-checked before narrowing to `u16` or `i16`.
- Protobuf floats must be finite.
- Pit command float values must represent whole-number values that fit into
  `u16`, because the underlying SDK pit commands are integer gallon/PSI values.
- Chat macros must be in the supported `1..=15` range.

Outbound mapping happens in `broadcast_service/response.rs`. Query responses
are direct live telemetry snapshots. Command responses either return observed
state after the command is accepted, or remain empty for ack-only commands whose
post-command state is not observed.

## Validation and Boundary Rules

The service boundary is the authoritative place for external input validation.
Generated protobuf types are not trusted just because they were decoded by
tonic.

Important validation helpers:

- `required_u16` / `optional_u16`: `uint32` fields narrowed to `u16`.
- `optional_i16`: `int32` fields narrowed to `i16`.
- `required_u32`: required `uint32` fields.
- `optional_string`: optional non-empty string.
- `required_enum`: required enum, rejects `UNKNOWN`, rejects invalid
  numeric enum values.
- `required_f32`: required finite float.
- `f32_to_u16`: finite whole-number float narrowed to `u16`.
- `required_chat_macro`: required chat macro in `1..=15`.

These checks protect the lower SDK layer from invalid gRPC inputs and produce
client-actionable `INVALID_ARGUMENT` responses.

## Error Handling

Input validation errors are returned as `INVALID_ARGUMENT`.

SDK errors from `iracing_sdk::Broadcast` are mapped by
`broadcast_error_to_status`:

| SDK error category | gRPC status | Rationale |
| --- | --- | --- |
| Connection failure | `UNAVAILABLE` | The broadcast channel cannot currently be used. |
| Unsupported platform | `FAILED_PRECONDITION` | The server cannot perform Windows-only work on this platform. |
| Windows API failure | `UNAVAILABLE` | The Win32 broadcast send path failed at runtime. |
| Retryable buffer or SDK error | `UNAVAILABLE` | Caller may retry after the simulator or environment recovers. |
| Other SDK error | `INTERNAL` | The request passed service validation but failed unexpectedly below the boundary. |

Each SDK error is logged with `tracing::warn!` and includes whether the SDK
classified it as retryable. Error messages should remain useful to operators but
must not include secrets or unrelated process details.

## Platform Boundaries

The service has two distinct platform surfaces:

- Generated protobuf types and raw tonic client/server types are
  cross-platform.
- `BroadcastService` and the real server implementation are Windows-only.

The platform split is enforced with `#[cfg(windows)]` in `lib.rs`,
`broadcast_service/`, and `server.rs`.

This mirrors the lower-level SDK design:

- `iracing_sdk::BroadcastCommand` is typed command data.
- `iracing_sdk::Broadcast` is the Windows transport that registers
  `IRSDK_BROADCASTMSG` and sends `SendNotifyMessageW(HWND_BROADCAST, ...)`.

Any future server feature that touches live iRacing shared memory or broadcast
messages must stay behind Windows gates. Protobuf definitions and generated
client/server types should remain portable.

## Build and Code Generation

The crate depends on:

- `tonic` for gRPC transport.
- `prost` and `tonic-prost` for protobuf message support.
- `tonic-prost-build` for build-time code generation.
- `protoc-bin-vendored` so local and CI builds do not depend on a system
  `protoc`.
- `tokio` for the async server runtime.
- `tracing` and `tracing-subscriber` for operational logging.

Build flow:

```text
cargo build
  -> build.rs runs
  -> vendored protoc path is configured
  -> proto/broadcast.proto is compiled
  -> generated Rust is placed in OUT_DIR
  -> tonic::include_proto! includes generated module
```

Because the generated code is derived from `broadcast.proto`, protocol changes
should be made in the proto first, followed by service implementation, tests,
and documentation updates.

## Operational Model

The default server bind address is local-only:

```text
[::1]:50051
```

Override it with:

```bash
BROADCAST_ADDR="0.0.0.0:50051" cargo run -p iracing-broadcast-grpc-service --bin iracing-broadcast-server
```

Operational considerations:

- Binding to a non-loopback address exposes simulator controls to the network.
  Put the service behind network controls, TLS termination, or an authenticated
  proxy before exposing it beyond the local machine.
- The service currently logs SDK broadcast failures but does not emit metrics.
  Production wrappers should add request counts, error counts, latency, and
  command audit logging where appropriate.
- gRPC calls are not idempotent in the general case. Retrying a command may send
  the command multiple times.
- `PitCommandStream` can partially apply commands before failing.
- The Win32 broadcast channel is asynchronous from the perspective of simulator
  state. A successful send does not guarantee that the simulator consumed or
  applied the command.

## Response Semantics

Responses fall into two groups:

- Query RPCs and observed command RPCs return live telemetry or session-derived
  state. If observation is disabled or the required telemetry variables are not
  available, these RPCs return `FAILED_PRECONDITION`.
- Ack-only command RPCs return an empty response after validation and command
  send succeeds. They do not prove that iRacing applied the command.
- Subscription RPCs return server streams of the same current-state responses as
  the unary query RPCs. A stream emits an initial snapshot, suppresses unchanged
  repeated values, and terminates with the same gRPC status mapping used by
  unary observed reads if observation is disabled, telemetry is unavailable, or
  the live source ends.

`VideoCapture` remains ack-only; `CurrentVideoCapture` is the read path for
observed capture state, and `SubscribeCurrentVideoCapture` is the streaming read
path. `PitCommandStream` can partially apply earlier commands before a later
stream item fails, and no rollback is attempted.

## Concurrency and Ordering

`BroadcastService` stores a single `iracing_sdk::Broadcast` client and tonic may
serve multiple RPCs concurrently. The SDK broadcast transport sends each command
through `SendNotifyMessageW` to `HWND_BROADCAST`; the crate does not currently
serialize commands across independent RPCs.

Ordering guarantees:

- Commands inside one `PitCommandStream` are processed sequentially in stream
  order.
- Independent unary RPCs may be processed concurrently by tonic.
- Subscription streams are independent server-to-client streams. They observe
  the live state source and apply backpressure through the gRPC stream; a slow
  subscriber may receive fewer intermediate state changes but will receive the
  latest changed value observed by its stream loop.
- iRacing's own handling order is outside this crate's direct control.

If future behavior requires strict global ordering, add an explicit command
queue in the service layer and document the latency and backpressure policy.

## Extending the Service

When adding a new broadcast command:

1. Add or update the protobuf enum, request, response, and RPC in
   `proto/broadcast.proto`.
2. Rebuild to generate updated tonic/prost types.
3. Add server-side validation and conversion in `broadcast_service/*`.
4. Add unit tests for conversion and validation, especially range checks and
   unsupported enum sentinels.
5. Update this architecture document if request lifecycle, response semantics,
   platform support, or operational behavior changes.
6. If the command exposes durable current state, add or update the matching
   unary `Current*` query and server-streaming `SubscribeCurrent*` RPC together.

When changing response semantics from command-acceptance to observed simulator
state, add characterization tests for the existing response shape first, then
introduce the state observation path behind a clearly documented behavior
change.

## Testing Strategy

Recommended checks for this crate:

```bash
cargo fmt --all -- --check
cargo test -p iracing-broadcast-grpc-service
cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings
```

Additional workspace gates before commit or push should match the repository
quality workflow:

```bash
python3 scripts/check_test_fixtures.py
cargo build --workspace
cargo test --workspace --all-targets
cargo check -p iracing-sdk --lib --target wasm32-unknown-unknown --all-features
```

For documentation-only changes, at minimum run formatting or a markdown sanity
check if one exists. For protocol, client, server, or public API changes, run
the crate tests and the relevant workspace quality gates.

## Known Gaps and Future Work

- Implement camera group discovery from session data or a live state source.
- Resolve camera switch responses to observed car index/group/camera values.
- Observe replay state after replay commands.
- Observe video capture or acknowledgement state if iRacing exposes one.
- Add integration tests with a fake broadcast transport so server validation and
  command conversion can be exercised off Windows.
- Add optional TLS/authentication guidance or first-class server configuration
  before recommending non-loopback deployments.
- Add metrics and structured command audit logs for operational support.
