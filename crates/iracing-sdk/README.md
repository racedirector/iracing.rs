# iracing-sdk

Low-level iRacing telemetry parsing utilities for Rust.

Native SDK wire definitions live in the dependency-light `iracing-irsdk` crate
and remain available here through the backward-compatible `iracing_sdk::irsdk`
namespace.

This crate provides:

- One completed-recording model, `IbtFile`, with immutable metadata, direct frame/range access, and deterministic `IbtReplay`.
- Source-independent telemetry layouts and decoding through `TelemetryLayout`, `VarData`, and `TelemetryValue`.
- Typed and dynamic adapters through `FramePacket`, `FrameAdapter`, `AdapterValidation`, and `DynamicFrame`.
- Session YAML decoding and parsing through `SessionInfoBytes` and `SessionInfo`.
- Windows shared-memory acquisition, live providers/connections, and broadcast tools.

## Start Here

1. Open a completed recording with `IbtFile::open(path)` or take ownership with `IbtFile::from_bytes(bytes)`.
2. Inspect `header()`, `disk_header()`, `variable_headers()`, `session_info_bytes()`, `physical_layout()`, and `telemetry_layout()`.
3. Read an indexed `frame(index)`, lazily traverse `frames(start..end)`, or own a replay cursor with `file.replay()`.
4. Decode recorded frames directly, or call `frame.into_packet()` when using existing typed/dynamic packet adapters.
5. On Windows, use `WindowsConnection` or `LiveConnection` for live telemetry.

Mapped recordings must remain unchanged and untruncated until the owning file
or replay drops. Use owned bytes when you cannot guarantee immutable storage.
Opening a mapped file establishes metadata once without copying its frame region.

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

## Recorded Frame Access

```rust,no_run
use iracing_sdk::IbtFile;

fn inspect() -> iracing_sdk::Result<()> {
    let file = IbtFile::open("telemetry.ibt")?;
    let first = file.frame(0)?;
    println!("record {}: {:?}", first.index(), first.value("Speed")?);
    for frame in file.frames(0..file.frame_count())? {
        let frame = frame?;
        println!("record {}: {:?}", frame.index(), frame.value("Speed")?);
    }
    Ok(())
}
```

Coordinates are zero-based `usize` physical record indices. Frame location is
O(1), ranges are half-open and lazy, and independent iterators share no cursor.
Range ends clamp to EOF; reversed bounds and starts beyond EOF fail. An iterator
read error consumes that coordinate; direct access can retry it. Every `IbtFrame`
retains the exact layout established by its file and owns its bytes after file drop.

## Pull-Driven Replay

```rust,no_run
use iracing_sdk::IbtFile;

fn replay() -> iracing_sdk::Result<()> {
    let mut replay = IbtFile::open("telemetry.ibt")?.replay();
    replay.set_range(10..20)?;
    replay.seek(12)?;
    while let Some(frame) = replay.next_frame()? {
        println!("record {}", frame.index());
    }
    Ok(())
}
```

`current_frame()` reads without advancing, `advance()` consumes one coordinate,
and `next_frame()` advances only after a successful read. EOF persists until a
seek or range reset. Replay has no wall-clock pacing or background subscription.
`file()` borrows the recording; `into_file()` recovers it.

## Session Parsing

```rust,no_run
use iracing_sdk::{IbtFile, schema::SessionInfo};

fn session() -> iracing_sdk::Result<()> {
    let file = IbtFile::open("telemetry.ibt")?;
    if let Some(bytes) = file.session_info_bytes() {
        let session = SessionInfo::try_from(bytes.clone())?;
        println!("Track: {}", session.weekend_info.track_display_name);
    }
    Ok(())
}
```

Session bytes preserve the complete advertised region, including padding.
Decoding honors the declared encoding and NUL boundary; typed parsing applies
the SDK's sanitization path. `sanitized_yaml()` returns cleaned text without
parsing or reserializing it and rejects empty text; it does not validate YAML
syntax. Session access is independent of traversal.

## Typed Recorded Rows

```rust,no_run
use iracing_sdk::{FrameAdapter, IRacingTelemetryFrame, IbtFile};

#[derive(IRacingTelemetryFrame)]
struct Row {
    #[field_name = "Speed"]
    #[fail_if_missing]
    speed: f32,
}

fn rows() -> iracing_sdk::Result<()> {
    let file = IbtFile::open("telemetry.ibt")?;
    let validation = Row::validate_layout(file.telemetry_layout())?;
    for frame in file.all_frames()? {
        let packet = frame?.into_packet()?;
        let row = Row::adapt(&packet, &validation);
        println!("{} m/s", row.speed);
    }
    Ok(())
}
```

The `derive` feature supplies the macro; handwritten adapters use the same
validation/extraction contract. `IbtFrame::into_packet()` is the single recorded
compatibility bridge: it shares bytes/layout and checks that the record index
and signed session revision fit the packet's `u32` fields. Native recorded
identity remains `usize` independently of those packet limits.

## Handwritten Adapters

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
        validation.ensure_packet(packet).expect("adapter layout mismatch");
        Self { speed: validation.decode(packet, 0).expect("decode").expect("required field") }
    }
}
```

## Live Telemetry (Windows Only)

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

## Features and Platforms

| Feature | Purpose |
| --- | --- |
| `derive` (default) | Re-export telemetry adapter derive macros. |
| `test-utils` | Fixture helpers for downstream tests. |
| `codegen` | JSON schema generation helpers. |
| `schema-discovery` | Preserve and inspect unknown session fields. |
| `benchmark` | Criterion targets and hidden delivery experiment infrastructure. |

Recorded file/frame/replay access, session models, and adapters are portable.
Actual live shared-memory acquisition and broadcast transport require Windows;
`LiveConnection` retains a portable unsupported-platform builder stub.
`Provider` remains source-independent and is used by the Windows live source.

## Examples and CLI

- `cargo ibt-read-frame --ibt-path ./session.ibt --csv-output-path ./positions.csv`
- `cargo manual-frame-adapter --ibt-path ./session.ibt --csv-output-path ./positions.csv`
- `cargo ibt-derive --ibt-path ./session.ibt --csv-output-path ./positions.csv`
- `cargo enum-bitfields-ibt --ibt-path ./session.ibt`
- Windows: `cargo live-subscribe --csv-output-path .\\positions.csv`
- Windows: `cargo session-updates --output-dir .\\sessions`
- Windows: `cargo session-updates --car-setup-only --output-dir .\\setups`

Omit `--output-dir` from `session-updates` to print the selected YAML payload to stdout.

The consolidated CLI lives in [`iracing-sdk-cli`](../iracing-sdk-cli):

```text
cargo iracing-sdk headers ibt --path ./session.ibt
cargo iracing-sdk variables ibt --path ./session.ibt
cargo iracing-sdk session snapshot ibt --path ./session.ibt
cargo iracing-sdk session schema ibt --path ./session.ibt
cargo iracing-sdk session discover ibt --path ./session.ibt
cargo iracing-sdk telemetry snapshot ibt --path ./session.ibt --index 0 --format json
cargo iracing-sdk telemetry convert --path ./session.ibt --format csv --output ./telemetry.csv
```

Windows live session commands retain their acquisition behavior:

```text
cargo iracing-sdk session schema live
cargo iracing-sdk session discover live
cargo iracing-sdk session snapshot live --output ./live-session.yaml
```

Use generated `--help` for current formats, range selection, live commands, and
broadcast composition. Session and document commands default to YAML on stdout;
`--format` and `--output` select representation and destination.

## Testing

Deterministic fixtures live under `test-data/ibt` and are inventoried by
`manifest.json`. Run `cargo test-fixtures` to generate, verify, and check drift.
Read [development checks](../../docs/development.md) for the broad gate, including
feature-gated benchmark-infrastructure tests. Historical benchmark identifiers
remain stable; they do not represent additional public recording models.
