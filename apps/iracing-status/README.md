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
