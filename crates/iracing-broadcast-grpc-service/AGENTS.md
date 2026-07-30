# AGENTS.md

Cheat sheet for agents editing `crates/iracing-broadcast-grpc-service`.

## Start Here

- Read `docs/architecture.md` before changing the protobuf contract, server implementation, client wrapper, response semantics, platform support, or operational behavior.
- This crate is a gRPC command-and-observation gateway for iRacing broadcast controls. Keep raw Win32 broadcast message packing in `iracing-sdk`; this crate validates gRPC input, orchestrates commands through application use cases, and observes supported state changes through live telemetry.
- Generated prost/tonic code comes from `proto/broadcast.proto` through `build.rs`; do not edit generated output in `OUT_DIR`.

## Critical Commands

- `cargo test -p iracing-broadcast-grpc-service` runs the crate's tests.
- `cargo check -p iracing-broadcast-grpc-service --all-targets` catches library, bin, and test drift.
- `cargo run -p iracing-broadcast-grpc-service --bin iracing-broadcast-server` starts the server on Windows; non-Windows exits with an unsupported-platform error.
- `BROADCAST_ADDR="[::1]:50051" cargo run -p iracing-broadcast-grpc-service --bin iracing-broadcast-server` overrides the bind address.
- Before committing broader changes, run the root quality gates from `/AGENTS.md`, especially formatting, clippy, and workspace tests.

## Crate Layout

- `proto/broadcast.proto`: public protocol contract. Add or change RPCs, messages, optional fields, and enum values here first.
- `build.rs`: compiles the proto using `tonic-prost-build` and vendored `protoc`; developers and CI should not need a system `protoc`.
- `src/lib.rs`: crate facade and generated type re-exports. `BroadcastService` is exported only on Windows.
- `src/broadcast_service/`: thin tonic adapter, request/command conversion, response mapping, error-to-status mapping, and composition builder.
- `src/broadcast_app/`: transport-independent models, ports, errors, and use-case orchestration.
- `src/broadcast_iracing/`: adapters from application ports to `iracing-sdk` command transport and live telemetry/session observation.
- `src/telemetry_observer.rs`: shared, serialized provider reads and typed telemetry snapshot/change observation.
- `src/bin/server.rs`: Windows-only runtime composition, dual-stack listener, tracing, gRPC health, and reflection.
- `docs/architecture.md`: authoritative local design notes, response semantics, extension flow, operational model, and known gaps.

## Platform Boundaries

- Keep generated protobuf types and raw tonic client/server types cross-platform. There is currently no separate ergonomic client wrapper.
- Keep the real `BroadcastService` implementation and any live iRacing broadcast or shared-memory work behind `#[cfg(windows)]`.
- The server can run only where the SDK broadcast transport can send Win32 messages. A non-Windows server binary must continue to fail clearly.

## Protocol & Validation Rules

- Treat `broadcast.proto` as the public API. Protocol changes need matching server conversion, client wrapper updates when useful, tests, and docs updates.
- Use optional primitive fields where absence means "retain current observed state"; use required fields where the command has no meaningful fallback.
- Reject `UNKNOWN` protobuf enum values and invalid numeric enum values before conversion to SDK types.
- Range-check narrow conversions (`u16`, `i16`), require finite floats, and keep chat macros in the supported `1..=15` range.
- Bound client streams before buffering them; `PitCommandStream` currently accepts at most 1000 commands.
- Map boundary errors to the gRPC statuses described in `docs/architecture.md`.

## Response Semantics

- State-bearing RPCs snapshot current state, send, and wait up to the configured observation timeout for a matching telemetry change. Ack-only RPCs only confirm validation and a successful synchronous send path.
- Optional state fields are resolved from the pre-command snapshot. Do not silently substitute protobuf defaults.
- `PitCommandStream` validates and buffers the bounded stream before use-case execution, then sends commands sequentially. Send or observation failure can still leave earlier commands applied; there is no rollback.

## Operational Guardrails

- The default bind address is dual-stack all-interfaces: `[::]:50051`.
- Exposing `BROADCAST_ADDR` on a non-loopback interface exposes simulator controls. Document or require network controls, TLS termination, or an authenticated proxy before recommending that deployment.
- gRPC command calls are generally not idempotent; retrying can send a command more than once.
- The server registers gRPC health and reflection (v1 and v1alpha) alongside the broadcast service.
- Avoid logging secrets or unrelated process details. SDK broadcast failures should stay operator-actionable and safe for logs.

## Testing Guidance

- Prefer portable tests for generated protocol shape. The service/application/adapter modules are currently Windows-gated, so their unit tests run only on Windows.
- Server behavior that depends on `iracing_sdk::Broadcast` is Windows-only; preserve cross-platform compilation with cfg gates.
- For new broadcast commands, cover enum sentinel rejection, optional/current-state semantics, required-field errors, numeric range checks, finite-float checks, observation timeout/error mapping, and response mapping.
- Protocol, public API, response-semantics, or operational changes should update `docs/architecture.md` in the same change.
