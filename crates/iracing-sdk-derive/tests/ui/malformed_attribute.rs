#[derive(iracing_sdk_derive::IRacingTelemetryFrame)]
struct Frame { #[field_name = 42] speed: f32 }
fn main() {}
