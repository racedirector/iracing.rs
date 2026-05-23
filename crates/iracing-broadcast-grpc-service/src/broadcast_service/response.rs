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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadcast_app::{AvailableCamera, AvailableCameraGroup};

    fn make_camera_selection(
        session_version: u32,
        car_index: u32,
        group: u32,
        camera: u32,
    ) -> CameraSelectionSnapshot {
        CameraSelectionSnapshot {
            session_version,
            car_index,
            group,
            camera,
        }
    }

    #[test]
    fn available_cameras_maps_all_fields() {
        let cameras = AvailableCameras {
            camera_groups: vec![
                AvailableCameraGroup {
                    number: 1,
                    name: "TV".to_string(),
                    cameras: vec![
                        AvailableCamera {
                            number: 2,
                            name: Some("Nose".to_string()),
                        },
                        AvailableCamera {
                            number: 3,
                            name: None,
                        },
                    ],
                },
                AvailableCameraGroup {
                    number: 4,
                    name: "Cockpit".to_string(),
                    cameras: vec![],
                },
            ],
            current: make_camera_selection(5, 10, 1, 2),
        };

        let response = available_cameras(cameras);

        assert_eq!(response.car_index, 10);
        assert_eq!(response.group, 1);
        assert_eq!(response.camera, 2);
        assert_eq!(response.camera_groups.len(), 2);

        let group0 = &response.camera_groups[0];
        assert_eq!(group0.number, 1);
        assert_eq!(group0.name, "TV");
        assert_eq!(group0.cameras.len(), 2);
        assert_eq!(group0.cameras[0].number, Some(2));
        assert_eq!(group0.cameras[0].name, Some("Nose".to_string()));
        assert_eq!(group0.cameras[1].number, Some(3));
        assert_eq!(group0.cameras[1].name, None);

        let group1 = &response.camera_groups[1];
        assert_eq!(group1.number, 4);
        assert_eq!(group1.name, "Cockpit");
        assert_eq!(group1.cameras.len(), 0);
    }

    #[test]
    fn available_cameras_with_empty_groups() {
        let cameras = AvailableCameras {
            camera_groups: vec![],
            current: make_camera_selection(1, 0, 0, 0),
        };
        let response = available_cameras(cameras);
        assert!(response.camera_groups.is_empty());
        assert_eq!(response.car_index, 0);
    }

    #[test]
    fn camera_switch_position_maps_snapshot_fields() {
        let snapshot = make_camera_selection(3, 99, 5, 7);
        let response = camera_switch_position(snapshot);
        assert_eq!(response.car_index, 99);
        assert_eq!(response.group, 5);
        assert_eq!(response.camera, 7);
    }

    #[test]
    fn camera_switch_number_maps_snapshot_fields() {
        let snapshot = make_camera_selection(3, 42, 6, 8);
        let response = camera_switch_number(snapshot);
        assert_eq!(response.car_index, 42);
        assert_eq!(response.group, 6);
        assert_eq!(response.camera, 8);
    }

    #[test]
    fn replay_set_play_speed_maps_speed_and_slow_motion() {
        let snapshot = ReplaySpeedSnapshot {
            speed: -2,
            is_slow_motion: true,
        };
        let response = replay_set_play_speed(snapshot);
        assert_eq!(response.speed, -2);
        assert!(response.is_slow_motion);

        let snapshot_normal = ReplaySpeedSnapshot {
            speed: 4,
            is_slow_motion: false,
        };
        let response_normal = replay_set_play_speed(snapshot_normal);
        assert_eq!(response_normal.speed, 4);
        assert!(!response_normal.is_slow_motion);
    }

    #[test]
    fn camera_set_state_maps_state_bits() {
        let snapshot = CameraStateSnapshot { state: 0b1010 };
        let response = camera_set_state(snapshot);
        assert_eq!(response.state, 0b1010);
    }

    #[test]
    fn replay_set_play_position_maps_frame_only() {
        let snapshot = ReplayPositionSnapshot {
            frame: 1234,
            session_number: 2,
            session_time: 99.5,
        };
        let response = replay_set_play_position(snapshot);
        assert_eq!(response.frame, 1234);
    }

    #[test]
    fn replay_search_maps_frame_session_number_and_session_time() {
        let snapshot = ReplayPositionSnapshot {
            frame: 500,
            session_number: 3,
            session_time: 12.5,
        };
        let response = replay_search(snapshot);
        assert_eq!(response.frame, 500);
        assert_eq!(response.session_number, 3);
        assert!((response.session_time - 12.5).abs() < f32::EPSILON);
    }

    #[test]
    fn pit_command_maps_all_fields() {
        let snapshot = PitServiceSnapshot {
            service_flags: 0b11,
            fuel: 10.5,
            lf_pressure: 1.1,
            rf_pressure: 2.2,
            lr_pressure: 3.3,
            rr_pressure: 4.4,
            tire_compound: 7,
        };
        let response = pit_command(snapshot);
        assert_eq!(response.service_flags, 0b11);
        assert!((response.fuel - 10.5).abs() < f32::EPSILON);
        assert!((response.lf_pressure - 1.1).abs() < 0.001);
        assert!((response.rf_pressure - 2.2).abs() < 0.001);
        assert!((response.lr_pressure - 3.3).abs() < 0.001);
        assert!((response.rr_pressure - 4.4).abs() < 0.001);
        assert_eq!(response.tire_compound, 7);
    }

    #[test]
    fn telemetry_command_maps_logging_flags() {
        let snapshot = TelemetryLoggingSnapshot {
            is_disk_logging_enabled: true,
            is_disk_logging_active: false,
        };
        let response = telemetry_command(snapshot);
        assert!(response.is_disk_logging_enabled);
        assert!(!response.is_disk_logging_active);

        let snapshot2 = TelemetryLoggingSnapshot {
            is_disk_logging_enabled: false,
            is_disk_logging_active: true,
        };
        let response2 = telemetry_command(snapshot2);
        assert!(!response2.is_disk_logging_enabled);
        assert!(response2.is_disk_logging_active);
    }

    #[test]
    fn force_feedback_maps_max_force() {
        let snapshot = ForceFeedbackSnapshot { max_force: 25.5 };
        let response = force_feedback(snapshot);
        assert!((response.max_force - 25.5).abs() < f32::EPSILON);
    }
}
