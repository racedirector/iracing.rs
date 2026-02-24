# iracing-sdk-codegen

Command-line schema/codegen utilities for the `iracing-sdk` workspace.

This crate provides binaries for generating telemetry and session schemas from:

- compile-time Rust types
- disk `.ibt` recordings
- live iRacing shared memory (Windows)

All schema outputs are YAML-serialized JSON Schema.

## Binaries

### `session-schema`

Generates the static baseline schema from `iracing_sdk::SessionInfo`.

```text
cargo run -p iracing-sdk-codegen --bin session-schema -- \
  --output-path ./session_schema.yml
```

### `disk-telemetry-schema`

Generates telemetry schema from an `.ibt` file's variable headers.

```text
cargo run -p iracing-sdk-codegen --bin disk-telemetry-schema -- \
  --ibt-path ./recording.ibt \
  --output-path ./disk_telemetry_schema.yml
```

### `live-telemetry-schema` (Windows)

Generates telemetry schema from live iRacing shared memory.

```text
cargo run -p iracing-sdk-codegen --bin live-telemetry-schema -- \
  --output-path ./live_telemetry_schema.yml \
  --allow-stale
```

### `disk-session-schema`

Generates session schema from session YAML embedded in an `.ibt` file.

Options:

- `--discover`: merge discovered unknown fields into emitted schema
- `--diff <PATH>`: compare against baseline schema
- `--diff-output-path <PATH>`: write full diff report (requires `--diff`)

```text
cargo run -p iracing-sdk-codegen --bin disk-session-schema -- \
  --ibt-path ./recording.ibt \
  --output-path ./disk_session_schema.yml \
  --discover \
  --diff ./session_schema.yml \
  --diff-output-path ./disk_session_diff.yml
```

### `live-session-schema` (Windows)

Generates session schema from live iRacing session YAML.

Options:

- `--allow-stale`: continue even if iRacing reports disconnected state
- `--discover`: merge discovered unknown fields into emitted schema
- `--diff <PATH>`: compare against baseline schema
- `--diff-output-path <PATH>`: write full diff report (requires `--diff`)

```text
cargo run -p iracing-sdk-codegen --bin live-session-schema -- \
  --output-path ./live_session_schema.yml \
  --allow-stale \
  --discover \
  --diff ./session_schema.yml \
  --diff-output-path ./live_session_diff.yml
```

### `car-setup-schema`

Generates car setup schema from either:

- `--ibt-path <FILE.ibt>` (all platforms)
- live iRacing session data when `--ibt-path` is omitted (Windows only)

```text
# Parse from IBT
cargo run -p iracing-sdk-codegen --bin car-setup-schema -- \
  --ibt-path ./recording.ibt \
  --output-dir ./out

# Parse from live iRacing (Windows)
cargo run -p iracing-sdk-codegen --bin car-setup-schema -- \
  --output-dir ./out
```

## Notes

- `live-*` bins (and live mode of `car-setup-schema`) require Windows because they depend on iRacing shared memory APIs.
- On non-Windows platforms, Windows-only modes return an explicit unsupported-platform error.
- This crate enables `iracing-sdk` features `codegen` and `schema-discovery` by default via dependency configuration.
- Diff mode is path/type oriented and ignores metadata-only changes (title/description changes alone do not produce diffs).
