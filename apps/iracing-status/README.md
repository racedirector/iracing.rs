# iRacing Status

Tauri + React status app for local iRacing connection state.

## HTTP API Routes

The source of truth for HTTP routes is [`openapi.yaml`](openapi.yaml). Do not add new HTTP routes directly to `src-tauri/src/server/http.rs`.

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
