# iracing-sdk

`iracing-sdk` is a command-line interface for inspecting iRacing telemetry recordings and, on Windows, working with live simulator telemetry. It provides user-facing workflows for headers, session data, variable metadata, telemetry snapshots, telemetry export, and live capture.

## Requirements

- A Rust toolchain compatible with this workspace (`rust-version = 1.88`) when building from source.
- `.ibt` file workflows are portable across the platforms supported by the workspace.
- Live shared-memory telemetry is **Windows-only** and requires a running iRacing simulator with an active telemetry connection.
- Live recording and the composed `broadcast` command are only present on Windows builds.

## Run from source

The workspace provides a Cargo alias for the executable:

```bash
git clone https://github.com/racedirector/iracing.rs
cd iracing.rs
cargo iracing-sdk --help
```

That alias is equivalent to:

```bash
cargo run -p iracing-sdk-cli --bin iracing-sdk -- --help
```

To build the executable directly:

```bash
cargo build -p iracing-sdk-cli --bin iracing-sdk --release
./target/release/iracing-sdk --help
```

On Windows, the built executable is `target\\release\\iracing-sdk.exe`.

To install the current checkout with Cargo:

```bash
cargo install --path crates/iracing-sdk-cli
iracing-sdk --help
```

The workspace is configured for tag-driven `cargo-dist` releases. Check the repository's GitHub Releases for packaged artifacts when available; running from the workspace checkout is the canonical path for current `main`.

## Quick start

Given an iRacing telemetry recording at `<FILE.ibt>`:

```bash
# Inspect the recording headers.
cargo iracing-sdk headers ibt --path <FILE.ibt>

# Decode the session information.
cargo iracing-sdk session snapshot ibt --path <FILE.ibt>

# Capture the first telemetry frame as JSON.
cargo iracing-sdk telemetry snapshot ibt --path <FILE.ibt> --index 0 --format json
```

For invocation syntax, use clap's generated help rather than this README as a command reference:

```bash
cargo iracing-sdk --help
cargo iracing-sdk session --help
cargo iracing-sdk session snapshot --help
cargo iracing-sdk telemetry --help
cargo iracing-sdk telemetry snapshot --help
```

If you are running a built or installed executable, replace `cargo iracing-sdk` with `iracing-sdk`.

## Representative workflows

### Inspect an IBT recording

Headers, decoded session information, and variable metadata can all be read from a recording without a running simulator:

```bash
cargo iracing-sdk headers ibt --path <FILE.ibt>
cargo iracing-sdk session snapshot ibt --path <FILE.ibt>
cargo iracing-sdk variables ibt --path <FILE.ibt>
```

Document-style commands write YAML by default. Use `--format` and `--output` when you need another supported representation or a file destination; `-` represents stdout. Use each command's `--help` output for the authoritative accepted formats and argument behavior.

### Generate a session schema

Generate a schema from the session information contained in a recording:

```bash
cargo iracing-sdk session schema ibt --path <FILE.ibt> --output session-schema.yml
```

On Windows, the same command family can operate on the current live session:

```powershell
cargo iracing-sdk session schema live --output live-session-schema.yml
```

### Capture one telemetry frame

Capture a zero-based frame from an IBT recording:

```bash
cargo iracing-sdk telemetry snapshot ibt --path <FILE.ibt> --index 11 --format json --output telemetry.json
```

On Windows, capture the next available live frame:

```powershell
cargo iracing-sdk telemetry snapshot live --format json --output telemetry.json
```

### Export recorded telemetry

Convert recorded frames to a stream-oriented output format:

```bash
cargo iracing-sdk telemetry convert --path <FILE.ibt> --format csv --output telemetry.csv
cargo iracing-sdk telemetry convert --path <FILE.ibt> --format jsonl --output telemetry.jsonl
```

Use `telemetry convert --help` for frame-range selection and the current output-format contract.

### Record live telemetry on Windows

With iRacing running and publishing telemetry:

```powershell
cargo iracing-sdk telemetry record --format jsonl --output telemetry.jsonl
```

Recording continues until the live source ends or you press Ctrl+C.

These are representative workflows, not an exhaustive command tree. Command names, arguments, defaults, validation, source selection, and output semantics are owned by the clap/source implementation and its tests.

## Where the pieces live

The CLI and telemetry stack are split by responsibility:

- [`src/main.rs`](src/main.rs) is the executable entry point and composes the application dependencies.
- [`src/commands`](src/commands) owns the clap command composition and command-specific behavior.
- [`src/utils.rs`](src/utils.rs) contains the CLI's source-selection helpers for disk and live telemetry.
- [`src/writer`](src/writer) owns CLI output formatting and destinations.
- [`iracing-sdk`](../iracing-sdk) owns telemetry/session reading, layouts, decoding, live integration, and the reusable SDK contracts used by this CLI.
- [`iracing-irsdk`](../iracing-irsdk) owns the dependency-light native iRacing wire definitions.
- [`iracing-broadcast-cli`](../iracing-broadcast-cli/README.md) documents the standalone Windows broadcast CLI; the Windows `iracing-sdk broadcast` branch delegates to that command implementation.

For the broader workspace map, development commands, and architecture links, start at the [workspace README](../../README.md) and [`docs/architecture`](../../docs/architecture/README.md).

Local Rust documentation can be generated with:

```bash
cargo doc -p iracing-sdk -p iracing-irsdk --no-deps
```

The clap declarations, command modules, SDK rustdoc, and tests remain authoritative for behavior and contracts; this README is only the user-facing orientation surface for `iracing-sdk`.
