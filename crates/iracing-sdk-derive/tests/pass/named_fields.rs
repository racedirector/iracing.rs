#[derive(iracing_sdk_derive::IRacingTelemetryFrame)]
struct Frame {
    #[field_name = "Speed"]
    speed: f32,
    #[field_name = "Gear"]
    gear: Option<i32>,
}
fn main() {}
