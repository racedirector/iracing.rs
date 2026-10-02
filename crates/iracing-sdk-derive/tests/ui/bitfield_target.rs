#[derive(iracing_sdk_derive::IRacingTelemetryFrame)]
struct Frame { #[bitfield(name = "SessionFlags", has = "1")] flags: f32 }
fn main() {}
