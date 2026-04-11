use iracing_sdk::IRacingTelemetryFrame;

#[derive(IRacingTelemetryFrame)]
#[allow(dead_code)]
pub struct DerivedFrameWithoutTracingDependency {
    #[field_name = "Speed"]
    speed: f32,
}
