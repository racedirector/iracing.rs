# iracing-sdk

Low-level iRacing telemetry parsing utilities for Rust.

Native SDK wire definitions live in the dependency-light `iracing-irsdk` crate
and remain available here through the backward-compatible `iracing_sdk::irsdk`
namespace.

This crate provides:

- Cross-platform `.ibt` indexed reads via `IbtReader` and replay via `IbtProvider`
- Streaming adapter primitives via `FramePacket`, `Provider`, `IbtProvider`, `DynamicFrame`, `FrameAdapter`, `AdapterValidation`, `FieldExtraction`, and `LayoutProvider`; `LiveProvider` is the Windows-only live source
- Session YAML parsing via `SessionInfo::parse`; telemetry session policies handle source-specific updates
- Type-safe telemetry extraction helpers (`TelemetryLayout`, `VarData`, `BitField`)
- Windows shared-memory access (`WindowsConnection`) when building on Windows

## Start Here

1. Use `IbtProvider` for sequential replay, or `IbtReader` for indexed frames and metadata snapshots (all platforms).
2. Use `Provider`/`IbtProvider` for frame-by-frame streaming; reach for `LiveProvider` on Windows when you want the live source.
3. For typed rows or ad-hoc per-frame decoding, reach for `FrameAdapter` or `DynamicFrame`.
4. Parse decoded session YAML with `SessionInfo::parse`; use telemetry session policies for live updates or IBT replay.
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
use iracing_sdk::{AdapterValidation, DynamicFrame, FrameAdapter, ibt::IbtReader};
```

## Quick Start

### Offline `.ibt` Replay (Cross-Platform)

```rust,no_run
use iracing_sdk::{LayoutProvider, VarData, provider::Provider, providers::ibt::IbtProvider};

async fn replay() -> iracing_sdk::Result<()> {
    let mut provider = IbtProvider::open("telemetry.ibt")?;
    let speed_info = provider
        .layout()
        .field_by_name("Speed").map(|(_, field)| field)
        .ok_or_else(|| iracing_sdk::IRacingSDKError::Parse {
            context: "schema lookup".to_string(),
            details: "missing Speed variable".to_string(),
        })?
        .clone();

    while let Some(packet) = provider.next_frame().await? {
        let speed_mps = f32::decode_field(packet.data(), &speed_info)?;
        let _speed_kph = speed_mps * 3.6;
    }

    Ok(())
}
```

### Session YAML Parsing

```rust,no_run
use iracing_sdk::{ibt::IbtReader, provider::SessionInformationProvider};

fn main() -> iracing_sdk::Result<()> {
    let reader = IbtReader::open("telemetry.ibt")?;
    if let Some(session) = reader.session_info()? {
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
            if let Some(frame) = connection.get_new_data()? {
                println!("Received {} bytes", frame.data.len());
            }
        }
        WaitResult::Timeout => {}
    }
    Ok(())
}
```

`get_new_data()` returns `Result<Option<LiveFrameSnapshot>>`. A successful
snapshot owns the accepted bytes together with their tick and session update
counter. Consume these fields together; no later shared-header read is needed.
Ordinary absence (including baseline/reset ticks and exhausted consistency
retries) returns `Ok(None)`; malformed live geometry or acquisition failures
return `Err`. Use `header_snapshot()` for an owned header observation.

### Streaming Adapters

```rust,no_run
use iracing_sdk::{AdapterValidation, FieldExtraction, FrameAdapter};

#[derive(serde::Serialize)]
struct Row {
    speed: f32,
}

impl FrameAdapter for Row {
    fn validate_layout(layout: &std::sync::Arc<iracing_sdk::TelemetryLayout>) -> iracing_sdk::Result<AdapterValidation> {
        let speed = AdapterValidation::resolve::<f32>(layout, "Speed", true)?.expect("required field");
        Ok(AdapterValidation::new(std::sync::Arc::clone(layout), vec![FieldExtraction::Required(speed)]))
    }

    fn adapt(packet: &iracing_sdk::FramePacket, validation: &AdapterValidation) -> Self {
        let frame = validation.for_packet(packet).expect("adapter layout mismatch");
        Self { speed: frame.decode(0).expect("decode").expect("required field") }
    }
}
```

## Features

| Feature | Purpose |
| --- | --- |
| `codegen` | Enables JSON schema generation helpers such as `session_root_schema`. |
| `derive` | Re-exports telemetry adapter derive macros from `iracing-sdk-derive`, including `IRacingTelemetryFrame`. |
| `schema-discovery` | Enables collection/overlay of unknown session fields (used with `codegen`). |
| `benchmark` | Enables benchmark targets. |

## Adapter Surface

- `FramePacket` — raw frame payload plus tick, session version, and a shared telemetry layout.
- `Provider` — frame source abstraction implemented by `IbtProvider`, with `LiveProvider` available only on Windows.
- `FrameAdapter` — two-phase validation/extraction trait for typed rows.
- `AdapterValidation`, `FieldExtraction`, `LayoutProvider` — adapter planning helpers.
- `DynamicFrame` — by-name lookup helper for debugging and exploratory analysis.

## Platform Matrix

| Capability | Linux/macOS | Windows |
| --- | --- | --- |
| `.ibt` replay (`IbtProvider`) | Yes | Yes |
| Session parsing (`SessionInfo::parse`) | Yes | Yes |
| `session schema type`, `session schema ibt`, and `session snapshot ibt` | Yes | Yes |
| `session schema live` and `session snapshot live` | No | Yes |
| Live shared memory (`WindowsConnection`) | No | Yes |
| `live-subscribe` and `session-updates` examples / `live-to-csv`, `live-to-jsonl`, and `live-json-snapshot` bins | No | Yes |

## Examples and Binaries

### Examples

- `ibt-read-frame` reads and decodes recorded frames directly:
  - `cargo ibt-read-frame --ibt-path ./session.ibt --csv-output-path ./positions.csv`
- `manual-frame-adapter` demonstrates a handwritten frame adapter:
  - `cargo manual-frame-adapter --ibt-path ./session.ibt --csv-output-path ./positions.csv`
- `ibt-subscribe` subscribes to typed recorded frames (requires `derive`):
  - `cargo ibt-subscribe --ibt-path ./session.ibt --csv-output-path ./positions.csv`
- `live-subscribe` subscribes to typed live frames (Windows only; requires `derive`):
  - `cargo live-subscribe --csv-output-path .\\positions.csv`
- `enum-bitfields-ibt` decodes enum and bitfield values from a recording:
  - `cargo enum-bitfields-ibt --ibt-path ./session.ibt`
- `session-updates` observes session YAML revisions (Windows only):
  - `cargo session-updates --output-dir .\\sessions`
  - `cargo session-updates --car-setup-only --output-dir .\\setups`
  - Omit `--output-dir` to print the selected YAML payload to stdout.

### Binaries

- `broadcast` sends iRacing broadcast commands (Windows only):
  - `cargo broadcast --help`
- `session` (requires `codegen,schema-discovery`; the `cargo session` alias enables both):
  - Type schema: `cargo session schema type`
  - Schema from an IBT recording: `cargo session schema ibt --path ./session.ibt`
  - Schema additions discovered from an IBT recording: `cargo session discover ibt --path ./session.ibt`
  - Session snapshot from an IBT recording: `cargo session snapshot ibt --path ./session.ibt --output ./session.yaml`
  - Live schema (Windows only): `cargo session schema live`
  - Live schema additions (Windows only): `cargo session discover live`
  - Live snapshot (Windows only): `cargo session snapshot live --output ./live-session.yaml`
  - All subcommands default to YAML on stdout. Use `--output -` for explicit stdout, `--output <file>` for a file, or `--encoding json` / `--encoding json-pretty` for JSON. Diagnostics go to stderr.
- `ibt-json-snapshot`:
  - `cargo run -p iracing-sdk-cli --bin ibt-json-snapshot -- --ibt-path ./session.ibt --output-path ./frame.jsonl [--frame-number 0]`
- `ibt-to-json`:
  - `cargo run -p iracing-sdk-cli --bin ibt-to-json -- --ibt-path ./session.ibt --output-path ./telemetry.jsonl`
- `live-to-csv` (Windows only):
  - `cargo run -p iracing-sdk-cli --bin live-to-csv -- --output-path .\\live.csv`
- `live-json-snapshot` (Windows only):
  - `cargo run -p iracing-sdk-cli --bin live-json-snapshot -- --output-path .\\live-snapshot.jsonl`
- `live-to-jsonl` (Windows only):
  - `cargo run -p iracing-sdk-cli --bin live-to-jsonl -- --output-path .\\live.jsonl`

## Troubleshooting

- Missing telemetry fixtures during tests:
  - Generated fixtures live under `test-data/ibt/` and are listed in `test-data/ibt/manifest.json`.
  - Run `cargo test-fixtures` from the repository root.
- `live-*` tools fail on non-Windows:
  - Live shared memory APIs are Windows-only.
- No session snapshot available:
  - `session snapshot` reports an error when the source contains no session information.
