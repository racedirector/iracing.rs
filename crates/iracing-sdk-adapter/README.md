# iracing-sdk-adapter

Streaming adapter layer over [`iracing-sdk`](../iracing-sdk) for consuming iRacing telemetry as a sequence of frames.

If you want to:

- replay an `.ibt` file as a stream of frames
- turn raw telemetry bytes into a typed “view” struct efficiently
- separate “schema validation” (slow, once) from “frame extraction” (fast, every frame)

…this crate is the entry point.

## What this crate provides

### `Provider` + `FramePacket`

`Provider` is a source of telemetry frames. You drive it in a loop by calling `next_frame()`, which yields a `FramePacket`:

- `data`: raw frame bytes (`Arc<[u8]>` for cheap cloning)
- `tick`: monotonic frame counter
- `session_version`: changes when session YAML changes
- `schema`: shared `VariableSchema` for decoding

When `session_version` changes, call `Provider::session_yaml(version)` to fetch updated session YAML (providers may return `Ok(None)` when unchanged).

Included providers:

- `IbtProvider` — cross-platform `.ibt` replay.
- `LiveProvider` — Windows-only shared memory provider (see caveats below).

### `FrameAdapter` (two-phase typed extraction)

`FrameAdapter` is the core abstraction for fast per-frame decoding:

1. `validate_schema(&VariableSchema) -> AdapterValidation` runs once and pre-computes an extraction plan (`FieldExtraction` offsets, types, etc).
2. `adapt(&FramePacket, &AdapterValidation) -> Self` runs for every frame and uses the cached plan (no per-frame `HashMap` lookups).

For ad-hoc exploration, `DynamicFrame` provides ergonomic by-name lookups (and deliberately trades performance for convenience).

## Quick start: replay an `.ibt` with `DynamicFrame`

```rust,no_run
use iracing_sdk_adapter::{DynamicFrame, FrameAdapter, IbtProvider, Provider};

fn main() -> iracing_sdk_adapter::Result<()> {
    let mut provider = IbtProvider::from_path("./recording.ibt")?;
    let schema = provider.schema();
    let validation = DynamicFrame::validate_schema(schema.as_ref())?;

    while let Some(packet) = provider.next_frame()? {
        let frame = DynamicFrame::adapt(&packet, &validation);
        if let Some(speed) = frame.f32("Speed") {
            println!("tick={} speed={}", packet.tick, speed);
        }
    }

    Ok(())
}
```

## Examples

Run these from the workspace root:

- Disk replay to CSV:
  - `cargo run -p iracing-sdk-adapter --example disk-position -- --help`
- Live replay to CSV (Windows-only):
  - `cargo run -p iracing-sdk-adapter --example live-position -- --help`
- Enum + bitfield decoding (Windows-only):
  - `cargo run -p iracing-sdk-adapter --example enum-bitfields-live -- --help`

## Benchmarks

- `cargo bench -p iracing-sdk-adapter --features benchmark`

## Windows notes

- `LiveProvider` is Windows-only. It currently supports fetching/cleaning session YAML via shared memory.
- Frame streaming for `LiveProvider` is evolving; check `LiveProvider::next_frame()` for the current behavior before using it in production code.
