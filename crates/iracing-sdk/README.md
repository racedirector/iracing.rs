# iracing-sdk

Low-level iRacing telemetry parsing utilities for Rust.

This crate provides:

- Cross-platform `.ibt` telemetry replay via `IbtReader`
- `.ibt` writing via `IbtWriter`
- Session YAML parsing and caching via `SessionInfo` and `SessionInfoParser`
- Type-safe telemetry extraction helpers (`VariableSchema`, `VarData`, `BitField`)
- Windows shared-memory access (`WindowsConnection`) when building on Windows

## Start Here

1. Use `IbtReader` for offline replay from `.ibt` files (all platforms).
2. Use `SessionInfoParser` for session YAML parsing/caching.
3. Use `WindowsConnection` for live telemetry on Windows.

## Install

Add the crate from your workspace or registry:

```toml
[dependencies]
iracing-sdk = "0.1"
```

Basic import:

```rust
use iracing_sdk::{FrameProjection, IbtReader, IbtWriter, SessionInfoParser, VarData};
```

## Quick Start

### Offline `.ibt` Replay (Cross-Platform)

```rust,no_run
use iracing_sdk::{IbtReader, VarData};

fn main() -> iracing_sdk::Result<()> {
    let mut reader = IbtReader::open("telemetry.ibt")?;
    let speed_info = reader
        .variables()
        .get_variable("Speed")
        .ok_or_else(|| iracing_sdk::IRacingSDKError::Parse {
            context: "schema lookup".to_string(),
            details: "missing Speed variable".to_string(),
        })?
        .clone();

    while let Some((frame, _tick, _session_version)) = reader.read_next_frame()? {
        let speed_mps = f32::from_bytes(&frame, &speed_info)?;
        let _speed_kph = speed_mps * 3.6;
    }

    Ok(())
}
```

### Session YAML Parsing

```rust,no_run
use iracing_sdk::{IbtReader, SessionInfo};

fn main() -> iracing_sdk::Result<()> {
    let reader = IbtReader::open("telemetry.ibt")?;
    if let Some(yaml) = reader.session_yaml()? {
        let session = SessionInfo::parse(&yaml)?;
        println!("Track: {}", session.weekend_info.track_display_name);
    }
    Ok(())
}
```

### Live Telemetry (Windows Only)

```rust,ignore
use iracing_sdk::{WaitResult, WindowsConnection};
use std::time::Duration;

fn main() -> iracing_sdk::Result<()> {
    let mut connection = WindowsConnection::try_connect()?;
    match connection.wait_for_update(Duration::from_millis(100))? {
        WaitResult::Signaled => {
            if let Some(frame) = connection.get_new_data() {
                println!("Received {} bytes", frame.len());
            }
        }
        WaitResult::Timeout => {}
    }
    Ok(())
}
```

### Writing Subset `.ibt` Files

```rust,no_run
use iracing_sdk::{FrameProjection, IbtReader, IbtWriteOptions, IbtWriter};

fn main() -> iracing_sdk::Result<()> {
    let mut reader = IbtReader::open("source.ibt")?;
    let projection = FrameProjection::from_variable_names(
        reader.variables(),
        ["SessionTime", "Speed", "RPM", "OnPitRoad"],
    )?;

    let options = IbtWriteOptions::from_reader(&reader)?;
    let mut writer = IbtWriter::create("subset.ibt", projection.target_schema().clone(), options)?;

    while let Some((frame, _, _)) = reader.read_next_frame()? {
        writer.write_projected_frame(&frame, &projection)?;
    }

    writer.finish()?;
    Ok(())
}
```

## Features

| Feature | Purpose |
|---|---|
| `codegen` | Enables JSON schema generation helpers such as `session_root_schema`. |
| `schema-discovery` | Enables collection/overlay of unknown session fields (used with `codegen`). |
| `tokio` | Enables async `wait_for_update_async` for Windows live telemetry. |
| `benchmark` | Enables benchmark targets. |

## Platform Matrix

| Capability | Linux/macOS | Windows |
|---|---|---|
| `.ibt` replay (`IbtReader`) | Yes | Yes |
| Session parsing (`SessionInfoParser`) | Yes | Yes |
| Live shared memory (`WindowsConnection`) | No | Yes |
| `live-position` example / `live-session-parser` and `live_to_csv` bins | No | Yes |

## Examples and Binaries

### Examples

- `disk-position`:
  - `cargo run -p iracing-sdk --example disk-position -- --ibt-path ./session.ibt --csv-output-path ./positions.csv`
- `live-position` (Windows only):
  - `cargo run -p iracing-sdk --example live-position -- --csv-output-path .\\positions.csv`

### Binaries

- `ibt-session-parser`:
  - `cargo run -p iracing-sdk --bin ibt-session-parser -- --ibt-path ./session.ibt --output-path ./session.yaml`
- `live-session-parser` (Windows only):
  - `cargo run -p iracing-sdk --bin live-session-parser -- --output-path .\\live-session.yaml`
- `live_to_csv` (Windows only):
  - `cargo run -p iracing-sdk --bin live_to_csv -- --output-path .\\live.csv`

## Troubleshooting

- Missing telemetry fixtures during tests:
  - Fixtures live under `test-data/` and are typically Git LFS assets.
  - Install Git LFS and run `git lfs pull`.
- `live-*` tools fail on non-Windows:
  - Live shared memory APIs are Windows-only.
- No session YAML written by parser tools:
  - `session_yaml()`/`session_info()` can legitimately return no content if unavailable.
