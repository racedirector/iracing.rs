# iracing-broadcast

`iracing-broadcast` is a Windows-only command-line interface for sending typed control and broadcast commands to iRacing. It is intended for operator and development workflows such as switching cameras, controlling replay playback, requesting pit service, managing disk telemetry, and triggering capture commands.

## Requirements

- **Windows**. The executable reports an unsupported-platform error on other operating systems.
- A Rust toolchain compatible with this workspace (`rust-version = 1.88`) when building from source.
- A running iRacing simulator for broadcast commands to have an effect.
- Commands that resolve live session metadata, such as `replay search-session-time`, additionally require an active iRacing shared-memory telemetry connection.

## Run from source

The workspace provides a Cargo alias for the standalone executable:

```powershell
git clone https://github.com/racedirector/iracing.rs
cd iracing.rs
cargo iracing-broadcast --help
```

That alias is equivalent to:

```powershell
cargo run -p iracing-broadcast-cli --bin iracing-broadcast -- --help
```

To build the executable directly:

```powershell
cargo build -p iracing-broadcast-cli --bin iracing-broadcast --release
.\target\release\iracing-broadcast.exe --help
```

The workspace is configured for tag-driven `cargo-dist` releases. Check the repository's GitHub Releases for packaged artifacts when available; running from the workspace checkout is the canonical path for current `main`.

## Quick start

With iRacing running on Windows, a few representative commands are:

```powershell
# Switch to car number 12 using the default camera group/camera.
cargo iracing-broadcast camera switch-number --car-number 12

# Pause replay playback.
cargo iracing-broadcast replay pause

# Start disk telemetry recording.
cargo iracing-broadcast telemetry start
```

For invocation syntax, use clap's generated help rather than this README as a command reference:

```powershell
cargo iracing-broadcast --help
cargo iracing-broadcast camera --help
cargo iracing-broadcast replay --help
cargo iracing-broadcast replay search-session-time --help
```

If you are running a built or installed executable, replace `cargo iracing-broadcast` with `iracing-broadcast`.

## Representative workflows

### Camera control

Switch to a specific race position, camera group, and camera index:

```powershell
cargo iracing-broadcast camera switch-position --position 1 --group 2 --camera 0
```

### Replay control

Jump to five minutes into the race session using the live session metadata published by iRacing:

```powershell
cargo iracing-broadcast replay search-session-time --session race --time 5:00
```

For direct replay operations that do not need session lookup, commands such as `replay pause`, `replay normal`, and `replay search <MODE>` use only the broadcast transport.

### Pit service

Request eight gallons of fuel:

```powershell
cargo iracing-broadcast pit fuel 8
```

### Capture and simulator utilities

```powershell
cargo iracing-broadcast video screenshot
cargo iracing-broadcast textures reload-all
cargo iracing-broadcast chat macro 1
```

These are examples, not an exhaustive command tree. Use the relevant `--help` output for the authoritative set of subcommands, arguments, defaults, and validation rules.

## Where the pieces live

The broadcast stack is split by responsibility:

- [`iracing-broadcast`](src/main.rs) is the executable entry point and composes the runtime dependencies used by commands.
- [`iracing-broadcast-cli`](src/lib.rs) owns the clap command composition and CLI-facing command behavior. The concrete command declarations live in [`src/commands.rs`](src/commands.rs).
- [`iracing-broadcast-sdk`](../iracing-broadcast-sdk) owns the typed broadcast command model and the Windows broadcast transport. See its [`Command`](../iracing-broadcast-sdk/src/command.rs) definitions and client implementation in [`src/client.rs`](../iracing-broadcast-sdk/src/client.rs).
- [`iracing-irsdk`](../iracing-irsdk) owns the dependency-light native iRacing wire definitions, including the raw broadcast message discriminators in [`src/broadcast.rs`](../iracing-irsdk/src/broadcast.rs).

For the broader workspace map, development commands, and architecture links, start at the [workspace README](../../README.md) and [`docs/architecture`](../../docs/architecture/README.md).

Local Rust documentation can be generated with:

```powershell
cargo doc -p iracing-broadcast-cli -p iracing-broadcast-sdk -p iracing-irsdk --no-deps
```

The clap/source definitions, SDK documentation, and raw wire definitions remain authoritative for behavior and contracts; this README is only the user-facing orientation surface for the standalone CLI.
