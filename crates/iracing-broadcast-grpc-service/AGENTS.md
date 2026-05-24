# AGENTS.md

Cheat sheet for agents editing `crates/iracing-broadcast-grpc-service`.

## Start Here

- Read `docs/architecture.md` before changing the protobuf contract, server implementation, client wrapper, response semantics, platform support, or operational behavior.
- This crate is a gRPC command gateway for iRacing broadcast controls. Keep raw Win32 broadcast message packing in `iracing-sdk`; this crate should validate gRPC input, convert to typed SDK commands, send through the SDK, and return command-acceptance responses.
- Generated prost/tonic code comes from `proto/broadcast.proto` through `build.rs`; do not edit generated output in `OUT_DIR`.

## Critical Commands

- `cargo test -p iracing-broadcast-grpc-service` runs the crate's tests.
- `cargo check -p iracing-broadcast-grpc-service --all-targets` catches library, bin, and test drift.
- `cargo run -p iracing-broadcast-grpc-service --bin iracing-broadcast-server` starts the server on Windows; non-Windows exits with an unsupported-platform error.
- `BROADCAST_ADDR="[::1]:50051" cargo run -p iracing-broadcast-grpc-service --bin iracing-broadcast-server` overrides the bind address.
- Before committing broader changes, run the root quality gates from `/AGENTS.md`, especially formatting, clippy, workspace tests, and the wasm compatibility check.

## Crate Layout

- `proto/broadcast.proto`: public protocol contract. Add or change RPCs, messages, optional fields, and enum values here first.
- `build.rs`: compiles the proto using `tonic-prost-build` and vendored `protoc`; developers and CI should not need a system `protoc`.
- `src/lib.rs`: crate facade and generated type re-exports. `BroadcastService` is exported only on Windows.
- `src/broadcast_service.rs`: Windows-only tonic server implementation, validation helpers, protobuf-to-SDK conversion, SDK error mapping, and RPC handling.
- `src/client.rs`: cross-platform ergonomic Rust client around the generated tonic client.
- `src/bin/server.rs`: `iracing-broadcast-server` entry point. The real server path is Windows-only.
- `docs/architecture.md`: authoritative local design notes, response semantics, extension flow, operational model, and known gaps.

## Platform Boundaries

- Keep generated protobuf types, raw tonic client/server types, and `BroadcastGrpcClient` cross-platform.
- Keep the real `BroadcastService` implementation and any live iRacing broadcast or shared-memory work behind `#[cfg(windows)]`.
- The server can run only where the SDK broadcast transport can send Win32 messages. A non-Windows server binary must continue to fail clearly.

## Protocol & Validation Rules

- Treat `broadcast.proto` as the public API. Protocol changes need matching server conversion, client wrapper updates when useful, tests, and docs updates.
- Use optional primitive fields where absence is meaningful so callers get actionable `INVALID_ARGUMENT` errors instead of silent defaults.
- Reject `UNKNOWN` protobuf enum values and invalid numeric enum values before conversion to SDK types.
- Range-check narrow conversions (`u16`, `i16`), require finite floats, and keep chat macros in the supported `1..=15` range.
- Map bad client input to `INVALID_ARGUMENT`; preserve the SDK error-to-gRPC status mapping described in `docs/architecture.md`.

## Response Semantics

- Current responses mean "request validated and command sent without a synchronous SDK error"; they are not authoritative simulator state.
- Many discovery/state fields are placeholders until the service observes live telemetry or session state. Do not present placeholder values as confirmed iRacing state.
- If changing a response from command acceptance to observed simulator state, add characterization coverage for the current shape first and update `docs/architecture.md`.
- `PitCommandStream` can partially apply earlier commands before a later stream item fails. Do not imply transactional rollback.

## Operational Guardrails

- The default bind address is loopback-only: `[::1]:50051`.
- Exposing `BROADCAST_ADDR` on a non-loopback interface exposes simulator controls. Document or require network controls, TLS termination, or an authenticated proxy before recommending that deployment.
- gRPC command calls are generally not idempotent; retrying can send a command more than once.
- Avoid logging secrets or unrelated process details. SDK broadcast failures should stay operator-actionable and safe for logs.

## Testing Guidance

- Prefer portable tests for proto/client conversion and validation helpers when possible.
- Server behavior that depends on `iracing_sdk::Broadcast` is Windows-only; preserve cross-platform compilation with cfg gates.
- For new broadcast commands, cover enum sentinel rejection, required-field errors, numeric range checks, finite-float checks, and ergonomic client mapping.
- Protocol, public API, response-semantics, or operational changes should update `docs/architecture.md` in the same change.
