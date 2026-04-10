# iracing-sdk-derive

Procedural macros for generating typed telemetry adapters for `iracing-sdk`.

## Supported field attributes

The `IRacingTelemetryFrame` derive macro recognizes these field-level attributes:

- `#[field_name = "..."]`
- `#[missing = "..."]`
- `#[fail_if_missing]`
- `#[calculated = "..."]`
- `#[skip]`
- `#[bitfield(name = "...", has = "...")]`
- `#[bitfield_map(name = "...", decoder = "...")]`

## Field strategies

- Required field: `#[field_name = "Speed"]`
- Optional field: `#[field_name = "Gear"]` on `Option<T>`
- Defaulted field: `#[field_name = "Fuel"] #[missing = "100.0"]`
- Critical field: `#[field_name = "Temp"] #[fail_if_missing]`
- Calculated field: `#[calculated = "std::time::Instant::now()"]`
- Skipped field: `#[skip]`
- Bitfield flag check: `#[bitfield(name = "SessionFlags", has = "iracing_sdk::SessionFlags::GREEN.bits()")]`
- Bitfield decoder: `#[bitfield_map(name = "SessionFlags", decoder = "iracing_sdk::session_dq_scoring_invalid")]`

## Example

```rust,ignore
use iracing_sdk_derive::IRacingTelemetryFrame;

#[derive(IRacingTelemetryFrame, Debug)]
struct CarData {
    #[field_name = "Speed"]
    speed: f32,

    #[field_name = "Gear"]
    gear: Option<i32>,

    #[field_name = "FuelLevel"]
    #[missing = "100.0"]
    fuel: f32,

    #[calculated = "std::time::Instant::now()"]
    timestamp: std::time::Instant,

    #[skip]
    last_lap_time: f32,

    #[bitfield(
        name = "SessionFlags",
        has = "iracing_sdk::SessionFlags::GREEN.bits()"
    )]
    is_green: bool,

    #[bitfield_map(
        name = "SessionFlags",
        decoder = "iracing_sdk::session_dq_scoring_invalid"
    )]
    dq_scoring_invalid: bool,
}
```

## Notes

- `#[bitfield(..., has = "...")]` only accepts `bool` or `Option<bool>`.
- `#[bitfield_map(..., decoder = "...")]` accepts any `T` or `Option<T>`.
- Both bitfield forms decode the telemetry variable as `iracing_sdk::BitField` first.
