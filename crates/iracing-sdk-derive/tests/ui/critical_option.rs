#[derive(iracing_sdk_derive::IRacingTelemetryFrame)]
struct Frame { #[field_name = "Speed"] #[fail_if_missing] speed: Option<f32> }
fn main() {}
