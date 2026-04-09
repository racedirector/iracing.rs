# iracing-sdk-codegen

Command-line schema/codegen utilities for the `iracing-sdk` workspace.

This crate provides binaries for generating telemetry and session schemas from:

- compile-time Rust types
- disk `.ibt` recordings
- live iRacing shared memory (Windows)

All schema outputs are YAML-serialized JSON Schema.

## Cargo aliases

The workspace root defines Cargo aliases in `.cargo/config.toml` for each codegen bin:

- `cargo session-schema`
- `cargo disk-variable-schema`
- `cargo live-variable-schema`
- `cargo disk-session-schema`
- `cargo live-session-schema`
- `cargo car-setup-schema`
- `cargo iracing-primitives-schema`

Each alias runs the corresponding `iracing-sdk-codegen` binary and forwards additional arguments.

## Binaries

### `session-schema`

Generates the static baseline schema from `iracing_sdk::SessionInfo`.

```text
cargo run -p iracing-sdk-codegen --bin session-schema -- \
  --output-path ./session-schema.yml
```

### `disk-variable-schema`

Generates telemetry schema from an `.ibt` file's variable headers.

```text
cargo run -p iracing-sdk-codegen --bin disk-variable-schema -- \
  --ibt-path ./recording.ibt \
  --output-path ./disk-variable-schema.yml \
  --annotate   # optional
```

Options:

- `--annotate`: annotate `irsdk_*` units with primitive enum/bitflag refs and inject used `$defs`.

### `live-variable-schema` (Windows)

Generates telemetry schema from live iRacing shared memory.

```text
cargo run -p iracing-sdk-codegen --bin live-variable-schema -- \
  --output-path ./live-variable-schema.yml \
  --allow-stale \
  --annotate   # optional
```

Options:

- `--allow-stale`: continue even when iRacing reports disconnected state.
- `--annotate`: annotate `irsdk_*` units with primitive enum/bitflag refs and inject used `$defs`.

### `disk-session-schema`

Generates session schema from session YAML embedded in an `.ibt` file.

Options:

- `--discover`: merge discovered unknown fields into emitted schema
- `--diff <PATH>`: compare against baseline schema
- `--diff-output-path <PATH>`: write full diff report (requires `--diff`)

```text
cargo run -p iracing-sdk-codegen --bin disk-session-schema -- \
  --ibt-path ./recording.ibt \
  --output-path ./disk-session-schema.yml \
  --discover \
  --diff ./session-schema.yml \
  --diff-output-path ./disk-session-diff.yml
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
  --output-path ./live-session-schema.yml \
  --allow-stale \
  --discover \
  --diff ./session-schema.yml \
  --diff-output-path ./live-session-diff.yml
```

### `car-setup-schema`

Generates car setup schema from either:

- `--ibt-path <FILE.ibt>` (all platforms)
- live iRacing session data when `--ibt-path` is omitted (Windows only; requires iRacing to be connected)

```text
# Parse from IBT
cargo run -p iracing-sdk-codegen --bin car-setup-schema -- \
  --ibt-path ./recording.ibt \
  --output-dir ./out

# Parse from live iRacing (Windows)
cargo run -p iracing-sdk-codegen --bin car-setup-schema -- \
  --output-dir ./out
```

Notes:

- If `--output-path` is not provided, the tool computes a filename from `CarID`/`SeriesID` found in the session YAML.
- Live mode currently requires an active iRacing connection; there is no `--allow-stale` flag for this binary.

### `iracing-primitives-schema`

Generates a YAML schema for the `irsdk_*` primitive wrappers exported by
`iracing-sdk::types` (enums and bitflags).

```text
cargo run -p iracing-sdk-codegen --bin iracing-primitives-schema -- \
  --output-path ./iracing-primitives-schema.yml
```

Output notes:

- Primitive enum defs include `x-irsdk-kind: enum` and `x-irsdk-values` (name/value pairs).
- Bitflag defs include `x-irsdk-kind: bitflags`, `x-irsdk-values`, and `x-irsdk-known-mask`.
- `IncidentFlags` includes `x-irsdk-masks`, `x-irsdk-report-codes`, and `x-irsdk-penalty-codes`.

## Notes

- `live-*` bins (and live mode of `car-setup-schema`) require Windows because they depend on iRacing shared memory APIs.
- On non-Windows platforms, Windows-only modes return an explicit unsupported-platform error.
- This crate enables `iracing-sdk` features `codegen` and `schema-discovery` by default via dependency configuration.
- Diff mode is path/type oriented and ignores metadata-only changes (title/description changes alone do not produce diffs).
