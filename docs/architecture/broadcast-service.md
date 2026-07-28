# Broadcast Service in the Workspace

The broadcast gRPC crate exposes remote control of iRacing while preserving the
SDK as the owner of Win32 message transport and telemetry decoding.

## Portable and Windows surfaces

`build.rs` compiles `proto/broadcast.proto` with vendored `protoc` and embeds a
descriptor set. Generated request/response types, the generated service
trait/server wrapper, and `RawBroadcastClient` are available cross-platform.

The application modules, iRacing adapters, concrete `BroadcastService`, and
server composition are currently gated to Windows because they depend on the
live SDK transport.

## Layering

```text
proto + tonic request
  -> broadcast_service
       request validation
       protobuf/SDK command mapping
       response/status mapping
  -> broadcast_app
       models
       ports
       use-case orchestration
  -> broadcast_iracing
       SDK command sender
       telemetry/session observation
  -> iracing-sdk
       Win32 broadcast
       live frames and session YAML
```

`BroadcastService` should stay thin. It records request metadata, validates
external types, calls one use case, and maps the result. Business sequencing
belongs in `BroadcastUseCases`; SDK-specific conversion belongs in
`broadcast_iracing`.

## State-bearing command pattern

For RPCs with observable state, a use case generally:

1. snapshots current telemetry/session state;
2. fills omitted request fields from that snapshot;
3. sends a typed `BroadcastCommand`;
4. waits up to the configured timeout for a changed state matching the
   expectation;
5. returns the observed snapshot.

Ack-only RPCs validate and send but do not claim observed simulator state.
Retries are generally not idempotent.

The observation adapter serializes reads from a shared live provider with a
Tokio mutex. Typed `FrameAdapter` structs decode camera, replay, pit, telemetry
logging, and force-feedback fields. Session YAML supplies camera catalog and
driver/car-number resolution.

## Runtime composition

On Windows the server binary:

- defaults `BROADCAST_ADDR` to `[::]:50051`;
- creates a dual-stack listener for an IPv6 address;
- constructs `LiveProvider` and injects it into `BroadcastServiceBuilder`;
- registers broadcast, health, reflection v1, and reflection v1alpha services;
- configures structured tracing from `RUST_LOG` or a detailed default.

The all-interfaces default exposes simulator controls to reachable networks.
There is no built-in TLS, authentication, or authorization. Deployment must
provide network controls or a secured proxy.

For detailed RPC semantics, validation/status mapping, partial-application
behavior, and extension steps, read the crate-local
[architecture document](../../crates/iracing-broadcast-grpc-service/docs/architecture.md).

The current Windows implementation has a known SDK facade mismatch:
`SendProvider` does not exist, and live/session types are imported from stale
root paths. The crate-local document records the exact current failures. Treat
the layering above as implemented structure, but do not treat the Windows build
as green until those imports and the provider contract are reconciled.
