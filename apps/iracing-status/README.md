# iRacing Status

Tauri + React status app for local iRacing connection state.

## HTTP API Routes

The source of truth for HTTP routes is [`openapi.yaml`](docs/specs/openapi.yaml). Do not add new HTTP routes directly to `src-tauri/src/server/http.rs`.

## WebSocket API Routes

The source of truth for WebSocket routes is [`asyncapi.yaml`](docs/specs/asyncapi.yaml). Keep the documented route list there even when a route is only reserved for future implementation.

Remote-client HTTP smoke checks are available as a Postman collection at
[`postman/iracing-status.postman_collection.json`](postman/iracing-status.postman_collection.json).
Import gRPC requests separately from
[`postman/iracing-status-grpc.postman_collection.json`](postman/iracing-status-grpc.postman_collection.json)
because Postman does not save gRPC requests into HTTP collections. Enable the
matching transports in the app settings, then update the collection host and
port variables.

Route workflow:

1. Update `openapi.yaml` with the new path, operation ID, request shape, and response shape.
2. Validate the spec:

   ```sh
   pnpm run openapi:validate
   ```

3. Regenerate the Rust Axum API crate:

   ```sh
   pnpm run openapi:generate
   ```

4. Implement the generated trait method in `src-tauri/src/server/http.rs`.
5. Run Rust checks from the repository root, at minimum:

   ```sh
   cargo check -p iracing-status
   cargo fmt --all -- --check
   ```

The generated crate under `src-tauri/generated/http-api` owns routing, request validation, and response enum definitions. The app-owned HTTP service only supplies behavior by implementing the generated API trait.

## gRPC Client Generation

The source of truth for the generated gRPC clients is
[`crates/iracing-broadcast-grpc-service/proto/broadcast.proto`](../../crates/iracing-broadcast-grpc-service/proto/broadcast.proto).
Do not hand-author frontend request/response types once the generated client is in use.

Generation workflow:

1. Install the required tools:

   - app dependencies via `pnpm install` so the local `buf` and `protoc-gen-es` binaries are available

2. Optionally verify the tooling without generating files:

   ```sh
   pnpm run grpc:check
   ```

3. Generate the browser client into `src/generated/grpc-web`:

   ```sh
   pnpm run grpc:generate
   ```

   This emits `src/generated/grpc-web/broadcast_pb.ts`.

4. Re-run the app checks you need, at minimum:

   ```sh
   pnpm build
   ```

Notes:

- Browser client generation uses Buf with `@bufbuild/protoc-gen-es` and `target=ts`.
- The npm scripts set `BUF_CACHE_DIR` to `.buf-cache` by default so Buf does not need to write to the user profile cache.
- The generated browser client is consumed through `@connectrpc/connect` and `@connectrpc/connect-web`.
- gRPC-Web does not support client-side or bidirectional streaming. `broadcast.proto` currently includes `PitCommandStream(stream PitCommandRequest)`, so that RPC is not used by the browser integration client.
- The generated browser client speaks the gRPC-Web protocol. The current in-process Tauri server still serves tonic gRPC, so runtime calls require a gRPC-Web-compatible transport layer or proxy.
