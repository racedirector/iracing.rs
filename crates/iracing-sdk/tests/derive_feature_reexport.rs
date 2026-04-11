#![cfg(feature = "derive")]

use iracing_sdk::{FrameAdapter, IRacingTelemetryFrame};

#[derive(IRacingTelemetryFrame)]
#[allow(dead_code)]
struct ReexportedFrame {
    #[field_name = "Speed"]
    speed: f32,
}

#[test]
fn derive_feature_reexports_telemetry_frame_macro() {
    fn assert_frame_adapter<T: FrameAdapter>() {}

    assert_frame_adapter::<ReexportedFrame>();
}
