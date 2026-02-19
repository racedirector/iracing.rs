# iracing-sdk-codegen

Command-line schema/codegen utilities for the `iracing-sdk` workspace.

This crate provides binaries for generating telemetry and session schemas from:
- compile-time Rust types
- disk `.ibt` recordings
- live iRacing shared memory (Windows)

All schema outputs are YAML-serialized JSON Schema.

## Binaries

### `session_schema`
Generates the static baseline schema from `iracing_sdk::SessionInfo`.

```text
cargo run -p iracing-sdk-codegen --bin session_schema -- \
  --output-path ./session_schema.yml
```

### `disk_telemetry_schema`
Generates telemetry schema from an `.ibt` file's variable headers.

```text
cargo run -p iracing-sdk-codegen --bin disk_telemetry_schema -- \
  --ibt-path ./recording.ibt \
  --output-path ./disk_telemetry_schema.yml
```

### `live_telemetry_schema` (Windows)
Generates telemetry schema from live iRacing shared memory.

```text
cargo run -p iracing-sdk-codegen --bin live_telemetry_schema -- \
  --output-path ./live_telemetry_schema.yml \
  --allow-stale
```

### `disk_session_schema`
Generates session schema from session YAML embedded in an `.ibt` file.

Options:
- `--discover`: merge discovered unknown fields into emitted schema
- `--diff <PATH>`: compare against baseline schema
- `--diff-output-path <PATH>`: write full diff report (requires `--diff`)

```text
cargo run -p iracing-sdk-codegen --bin disk_session_schema -- \
  --ibt-path ./recording.ibt \
  --output-path ./disk_session_schema.yml \
  --discover \
  --diff ./session_schema.yml \
  --diff-output-path ./disk_session_diff.yml
```

### `live_session_schema` (Windows)
Generates session schema from live iRacing session YAML.

Options:
- `--allow-stale`: continue even if iRacing reports disconnected state
- `--discover`: merge discovered unknown fields into emitted schema
- `--diff <PATH>`: compare against baseline schema
- `--diff-output-path <PATH>`: write full diff report (requires `--diff`)

```text
cargo run -p iracing-sdk-codegen --bin live_session_schema -- \
  --output-path ./live_session_schema.yml \
  --allow-stale \
  --discover \
  --diff ./session_schema.yml \
  --diff-output-path ./live_session_diff.yml
```

## Notes

- `live_*` bins require Windows because they depend on iRacing shared memory APIs.
- This crate enables `iracing-sdk` features `codegen` and `schema-discovery` by default via dependency configuration.
- Diff mode is path/type oriented and ignores metadata-only changes (title/description changes alone do not produce diffs).

