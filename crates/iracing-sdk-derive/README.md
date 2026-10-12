# iracing-sdk-derive

Procedural macros for typed telemetry adapters, normally accessed through
`iracing-sdk` with its `derive` feature.

Start with the `IRacingTelemetryFrame` Rustdoc in [src/lib.rs](src/lib.rs)
for current macro behavior. Build local API documentation with
`cargo doc -p iracing-sdk-derive --no-deps --open`. A representative
named-field input is:

```rust,ignore
#[derive(iracing_sdk_derive::IRacingTelemetryFrame)]
struct Frame {
    #[field_name = "Speed"]
    speed: f32,
    #[field_name = "Gear"]
    gear: Option<i32>,
}
```

The [pass cases](tests/pass/) and [compile-fail cases](tests/ui/) own tested
accepted/rejected forms and diagnostics; the [test runner](tests/ui.rs)
selects those cases. The parser and generation code in [src/lib.rs](src/lib.rs)
own their respective decisions. Keep exact syntax and diagnostic expectations
with these executable owners rather than adding a second attribute matrix here.

Run `cargo test -p iracing-sdk-derive` for parser and UI coverage, and
`cargo test -p iracing-sdk-derive --doc` for public examples. Preserve generated
`.stderr` expectations when changing documentation.
