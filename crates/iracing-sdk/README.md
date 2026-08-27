# iracing-sdk

Low-level iRacing telemetry parsing utilities for Rust.

This crate provides:

- Cross-platform `.ibt` telemetry replay via `IbtReader`
- Streaming adapter primitives via `FramePacket`, `Provider`, `IbtProvider`, `DynamicFrame`, `FrameAdapter`, `AdapterValidation`, `FieldExtraction`, and `SchemaProvider`; `LiveProvider` is the Windows-only live source
- Session YAML parsing and caching via `SessionInfo` and `SessionInfoParser`
- Type-safe telemetry extraction helpers (`VariableSchema`, `VarData`, `BitField`)
- Windows shared-memory access (`WindowsConnection`) when building on Windows

## Start Here

1. Use `IbtReader` for offline replay from `.ibt` files (all platforms).
2. Use `Provider`/`IbtProvider` for frame-by-frame streaming; reach for `LiveProvider` on Windows when you want the live source.
3. For typed rows or ad-hoc per-frame decoding, reach for `FrameAdapter` or `DynamicFrame`.
4. For session YAML parsing and caching, rely on `SessionInfoParser`.
5. On Windows, use `WindowsConnection` for live telemetry.

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
use iracing_sdk::{AdapterValidation, DynamicFrame, FrameAdapter, reader::ibt::IbtReader};
```

## Quick Start

### Offline `.ibt` Replay (Cross-Platform)

```rust,no_run
use iracing_sdk::{VarData, VariableSchema, reader::ibt::IbtReader};

fn main() -> iracing_sdk::Result<()> {
    let mut reader = IbtReader::open("telemetry.ibt")?;
    let schema = VariableSchema::from_reader(&reader)?;
    let speed_info = schema
        .get_variable("Speed")
        .ok_or_else(|| iracing_sdk::IRacingSDKError::Parse {
            context: "schema lookup".to_string(),
            details: "missing Speed variable".to_string(),
        })?
        .clone();

    while let Some(frame) = reader.read_next_frame()? {
        let frame: Vec<u8> = frame.into_buffer().into();
        let speed_mps = f32::from_bytes(&frame, &speed_info)?;
        let _speed_kph = speed_mps * 3.6;
    }

    Ok(())
}
```

### Session YAML Parsing

```rust,no_run
use iracing_sdk::{reader::ibt::IbtReader, schema::SessionInfo, yaml_utils};

fn main() -> iracing_sdk::Result<()> {
    let reader = IbtReader::open("telemetry.ibt")?;
    if let Some(buffer) = reader.session_info_buffer()? {
        let raw_yaml: String = buffer.try_into()?;
        let yaml = yaml_utils::preprocess_iracing_yaml(&raw_yaml)?;
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
| `derive` | Re-exports telemetry adapter derive macros from `iracing-sdk-derive`, including `IRacingTelemetryFrame`. |
| `schema-discovery` | Enables collection/overlay of unknown session fields (used with `codegen`). |
| `benchmark` | Enables benchmark targets. |

## Adapter Surface

- `FramePacket` — raw frame payload plus tick, session version, and schema.
- `Provider` — frame source abstraction implemented by `IbtProvider`, with `LiveProvider` available only on Windows.
- `FrameAdapter` — two-phase validation/extraction trait for typed rows.
- `AdapterValidation`, `FieldExtraction`, `DefaultValue`, `SchemaProvider` — adapter planning helpers.
- `DynamicFrame` — by-name lookup helper for debugging and exploratory analysis.

## Platform Matrix

| Capability | Linux/macOS | Windows |
|---|---|---|
| `.ibt` replay (`IbtReader`) | Yes | Yes |
| Session parsing (`SessionInfoParser`) | Yes | Yes |
| Live shared memory (`WindowsConnection`) | No | Yes |
| `live-position` example / `live-session-parser`, `live-to-csv`, `live-to-jsonl`, and `live-json-snapshot` bins | No | Yes |

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
- `ibt-json-snapshot`:
  - `cargo run -p iracing-sdk --bin ibt-json-snapshot -- --ibt-path ./session.ibt --output-path ./frame.jsonl [--frame-number 0]`
- `ibt-to-json`:
  - `cargo run -p iracing-sdk --bin ibt-to-json -- --ibt-path ./session.ibt --output-path ./telemetry.jsonl`
- `live-session-parser` (Windows only):
  - `cargo run -p iracing-sdk --bin live-session-parser -- --output-path .\\live-session.yaml`
- `live-to-csv` (Windows only):
  - `cargo run -p iracing-sdk --bin live-to-csv -- --output-path .\\live.csv`
- `live-json-snapshot` (Windows only):
  - `cargo run -p iracing-sdk --bin live-json-snapshot -- --output-path .\\live-snapshot.jsonl`
- `live-to-jsonl` (Windows only):
  - `cargo run -p iracing-sdk --bin live-to-jsonl -- --output-path .\\live.jsonl`

## Troubleshooting

- Missing telemetry fixtures during tests:
  - Generated fixtures live under `test-data/ibt/` and are listed in `test-data/ibt/manifest.json`.
  - Run `python3 scripts/check_test_fixtures.py` from the repository root.
- `live-*` tools fail on non-Windows:
  - Live shared memory APIs are Windows-only.
- No session YAML written by parser tools:
  - `session_info_buffer()` can legitimately return no content if unavailable.
