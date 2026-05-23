use crate::{
    broadcast::{
        CameraDetail, CameraGroup, CameraSetStateResponse, CameraSwitchNumberResponse,
        CameraSwitchPositionResponse, ForceFeedbackCommandResponse, GetAvailableCamerasResponse,
        PitCommandResponse, ReplaySearchResponse, ReplaySetPlayPositionResponse,
        ReplaySetPlaySpeedResponse, TelemetryCommandResponse,
    },
    broadcast_app::{
        AvailableCameras, CameraSelectionSnapshot, CameraStateSnapshot, ForceFeedbackSnapshot,
        PitServiceSnapshot, ReplayPositionSnapshot, ReplaySpeedSnapshot, TelemetryLoggingSnapshot,
    },
};

pub(crate) fn available_cameras(cameras: AvailableCameras) -> GetAvailableCamerasResponse {
    GetAvailableCamerasResponse {
        camera_groups: cameras
            .camera_groups
            .into_iter()
            .map(|group| CameraGroup {
                number: group.number,
                name: group.name,
                cameras: group
                    .cameras
                    .into_iter()
                    .map(|camera| CameraDetail {
                        number: Some(camera.number),
                        name: camera.name,
                    })
                    .collect(),
            })
            .collect(),
        car_index: cameras.current.car_index,
        group: cameras.current.group,
        camera: cameras.current.camera,
    }
}

pub(crate) fn camera_switch_position(
    snapshot: CameraSelectionSnapshot,
) -> CameraSwitchPositionResponse {
    CameraSwitchPositionResponse {
        car_index: snapshot.car_index,
        group: snapshot.group,
        camera: snapshot.camera,
    }
}

pub(crate) fn camera_switch_number(
    snapshot: CameraSelectionSnapshot,
) -> CameraSwitchNumberResponse {
    CameraSwitchNumberResponse {
        car_index: snapshot.car_index,
        group: snapshot.group,
        camera: snapshot.camera,
    }
}

pub(crate) fn replay_set_play_speed(snapshot: ReplaySpeedSnapshot) -> ReplaySetPlaySpeedResponse {
    ReplaySetPlaySpeedResponse {
        speed: snapshot.speed,
        is_slow_motion: snapshot.is_slow_motion,
    }
}

pub(crate) fn camera_set_state(snapshot: CameraStateSnapshot) -> CameraSetStateResponse {
    CameraSetStateResponse {
        state: snapshot.state,
    }
}

pub(crate) fn replay_set_play_position(
    snapshot: ReplayPositionSnapshot,
) -> ReplaySetPlayPositionResponse {
    ReplaySetPlayPositionResponse {
        frame: snapshot.frame,
    }
}

pub(crate) fn replay_search(snapshot: ReplayPositionSnapshot) -> ReplaySearchResponse {
    ReplaySearchResponse {
        frame: snapshot.frame,
        session_number: snapshot.session_number,
        session_time: snapshot.session_time,
    }
}

pub(crate) fn pit_command(snapshot: PitServiceSnapshot) -> PitCommandResponse {
    PitCommandResponse {
        service_flags: snapshot.service_flags,
        fuel: snapshot.fuel,
        lf_pressure: snapshot.lf_pressure,
        rf_pressure: snapshot.rf_pressure,
        lr_pressure: snapshot.lr_pressure,
        rr_pressure: snapshot.rr_pressure,
        tire_compound: snapshot.tire_compound,
    }
}

pub(crate) fn telemetry_command(snapshot: TelemetryLoggingSnapshot) -> TelemetryCommandResponse {
    TelemetryCommandResponse {
        is_disk_logging_enabled: snapshot.is_disk_logging_enabled,
        is_disk_logging_active: snapshot.is_disk_logging_active,
    }
}

pub(crate) fn force_feedback(snapshot: ForceFeedbackSnapshot) -> ForceFeedbackCommandResponse {
    ForceFeedbackCommandResponse {
        max_force: snapshot.max_force,
    }
}
