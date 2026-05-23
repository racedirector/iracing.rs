#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AvailableCamera {
    pub(crate) number: u32,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AvailableCameraGroup {
    pub(crate) number: u32,
    pub(crate) name: String,
    pub(crate) cameras: Vec<AvailableCamera>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AvailableCameras {
    pub(crate) camera_groups: Vec<AvailableCameraGroup>,
    pub(crate) current: CameraSelectionSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CameraSelectionSnapshot {
    pub(crate) session_version: u32,
    pub(crate) car_index: u32,
    pub(crate) group: u32,
    pub(crate) camera: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CameraSelectionExpectation {
    pub(crate) car_index: Option<u32>,
    pub(crate) group: u32,
    pub(crate) camera: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CameraStateSnapshot {
    pub(crate) state: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CameraStateExpectation {
    pub(crate) state: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReplaySpeedSnapshot {
    pub(crate) speed: i32,
    pub(crate) is_slow_motion: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReplaySpeedExpectation {
    pub(crate) speed: i32,
    pub(crate) is_slow_motion: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReplayPositionSnapshot {
    pub(crate) frame: u32,
    pub(crate) session_number: u32,
    pub(crate) session_time: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplayPositionExpectation {
    AnyChange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PitServiceSnapshot {
    pub(crate) service_flags: u32,
    pub(crate) fuel: f32,
    pub(crate) lf_pressure: f32,
    pub(crate) rf_pressure: f32,
    pub(crate) lr_pressure: f32,
    pub(crate) rr_pressure: f32,
    pub(crate) tire_compound: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PitServiceExpectation {
    AnyChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TelemetryLoggingSnapshot {
    pub(crate) is_disk_logging_enabled: bool,
    pub(crate) is_disk_logging_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TelemetryLoggingExpectation {
    pub(crate) is_disk_logging_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ForceFeedbackSnapshot {
    pub(crate) max_force: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ForceFeedbackExpectation {
    pub(crate) max_force: f32,
}
