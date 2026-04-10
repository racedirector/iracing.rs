# iracing-sdk

Low-level iRacing telemetry parsing utilities for Rust.

This crate provides:

- Cross-platform `.ibt` telemetry replay via `IbtReader`
- Streaming adapter primitives via `FramePacket`, `Provider`, `IbtProvider`, `LiveProvider`, `DynamicFrame`, `FrameAdapter`, `AdapterValidation`, `FieldExtraction`, and `SchemaProvider`
- Session YAML parsing and caching via `SessionInfo` and `SessionInfoParser`
- Type-safe telemetry extraction helpers (`VariableSchema`, `VarData`, `BitField`)
- Windows shared-memory access (`WindowsConnection`) when building on Windows

## Start Here

1. Use `IbtReader` for offline replay from `.ibt` files (all platforms).
2. Use `Provider`/`IbtProvider`/`LiveProvider` when you want frame-by-frame streaming.
3. Use `FrameAdapter` or `DynamicFrame` when you want typed or ad-hoc per-frame decoding.
4. Use `SessionInfoParser` for session YAML parsing/caching.
5. Use `WindowsConnection` for live telemetry on Windows.

## Install

In this workspace, depend on the crate via a path dependency:

```toml
[dependencies]
iracing-sdk = { path = "../iracing-sdk" }
```

If you’re consuming this crate outside the workspace, use a git dependency (or a published
version if/when one exists):

```toml
[dependencies]
iracing-sdk = { git = "https://github.com/racedirector/iracing.rs", package = "iracing-sdk" }
```

Basic import:

```rust
use iracing_sdk::{AdapterValidation, DynamicFrame, FrameAdapter, IbtReader};
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

### Streaming Adapters

```rust,no_run
use iracing_sdk::{AdapterValidation, FieldExtraction, FrameAdapter};

#[derive(serde::Serialize)]
struct Row {
    speed: f32,
}

impl FrameAdapter for Row {
    fn validate_schema(schema: &iracing_sdk::VariableSchema) -> iracing_sdk::Result<AdapterValidation> {
        let speed_info = schema.get_variable("Speed").ok_or_else(|| iracing_sdk::IRacingSDKError::Parse {
            context: "Field validation".to_string(),
            details: "Missing required field 'Speed'".to_string(),
        })?;

        Ok(AdapterValidation::new(vec![FieldExtraction::Required {
            name: "Speed".to_string(),
            var_info: speed_info.clone(),
        }]))
    }

    fn adapt(packet: &iracing_sdk::FramePacket, validation: &AdapterValidation) -> Self {
        Self { speed: validation.fetch_or_default(packet, "Speed") }
    }
}
```

## Features

| Feature | Purpose |
|---|---|
| `codegen` | Enables JSON schema generation helpers such as `session_root_schema`. |
| `schema-discovery` | Enables collection/overlay of unknown session fields (used with `codegen`). |
| `tokio` | Enables async `wait_for_update_async` for Windows live telemetry. |
| `benchmark` | Enables benchmark targets. |

## Adapter Surface

- `FramePacket` — raw frame payload plus tick, session version, and schema.
- `Provider` — frame source abstraction implemented by `IbtProvider` and `LiveProvider`.
- `FrameAdapter` — two-phase validation/extraction trait for typed rows.
- `AdapterValidation`, `FieldExtraction`, `DefaultValue`, `SchemaProvider` — adapter planning helpers.
- `DynamicFrame` — by-name lookup helper for debugging and exploratory analysis.

## Platform Matrix

| Capability | Linux/macOS | Windows |
|---|---|---|
| `.ibt` replay (`IbtReader`) | Yes | Yes |
| Session parsing (`SessionInfoParser`) | Yes | Yes |
| Live shared memory (`WindowsConnection`) | No | Yes |
| `live-position` example / `live-session-parser` and `live-to-csv` bins | No | Yes |

## Examples and Binaries

### Examples

- `disk-position`:
  - `cargo run -p iracing-sdk --example disk-position -- --ibt-path ./session.ibt --csv-output-path ./positions.csv`
- `live-position` (Windows only):
  - `cargo run -p iracing-sdk --example live-position -- --csv-output-path .\\positions.csv`
- `adapter_disk_position`:
  - `cargo run -p iracing-sdk --example adapter_disk_position -- --ibt-path ./session.ibt --csv-output-path ./positions.csv`
- `adapter_live_position` (Windows only):
  - `cargo run -p iracing-sdk --example adapter_live_position -- --csv-output-path .\\positions.csv`
- `adapter_enum_bitfields_live` (Windows only):
  - `cargo run -p iracing-sdk --example adapter_enum_bitfields_live -- --max-frames 120`

### Binaries

- `ibt-session-parser`:
  - `cargo run -p iracing-sdk --bin ibt-session-parser -- --ibt-path ./session.ibt --output-path ./session.yaml`
- `live-session-parser` (Windows only):
  - `cargo run -p iracing-sdk --bin live-session-parser -- --output-path .\\live-session.yaml`
- `live-to-csv` (Windows only):
  - `cargo run -p iracing-sdk --bin live-to-csv -- --output-path .\\live.csv`

## Troubleshooting

- Missing telemetry fixtures during tests:
  - Fixtures live under `test-data/` and are typically Git LFS assets.
  - Install Git LFS and run `git lfs pull`.
- `live-*` tools fail on non-Windows:
  - Live shared memory APIs are Windows-only.
- No session YAML written by parser tools:
  - `session_yaml()`/`session_info()` can legitimately return no content if unavailable.
