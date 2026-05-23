use std::time::Duration;

use async_trait::async_trait;
use iracing_sdk::BroadcastCommand;

use super::{
    AvailableCameraGroup, BroadcastError, CameraSelectionExpectation, CameraSelectionSnapshot,
    CameraStateExpectation, CameraStateSnapshot, ForceFeedbackExpectation, ForceFeedbackSnapshot,
    PitServiceExpectation, PitServiceSnapshot, ReplayPositionExpectation, ReplayPositionSnapshot,
    ReplaySpeedExpectation, ReplaySpeedSnapshot, TelemetryLoggingExpectation,
    TelemetryLoggingSnapshot,
};

#[async_trait]
pub(crate) trait BroadcastCommandPort: Send + Sync {
    async fn send(&self, command: BroadcastCommand) -> Result<(), BroadcastError>;
}

#[async_trait]
pub(crate) trait CameraStatePort: Send + Sync {
    async fn selection_snapshot(&self) -> Result<CameraSelectionSnapshot, BroadcastError>;

    async fn wait_for_selection(
        &self,
        previous: CameraSelectionSnapshot,
        expected: CameraSelectionExpectation,
        timeout: Duration,
    ) -> Result<CameraSelectionSnapshot, BroadcastError>;

    async fn state_snapshot(&self) -> Result<CameraStateSnapshot, BroadcastError>;

    async fn wait_for_state(
        &self,
        previous: CameraStateSnapshot,
        expected: CameraStateExpectation,
        timeout: Duration,
    ) -> Result<CameraStateSnapshot, BroadcastError>;

    async fn available_camera_groups(
        &self,
        session_version: u32,
    ) -> Result<Vec<AvailableCameraGroup>, BroadcastError>;

    async fn resolve_car_index_by_number(
        &self,
        session_version: u32,
        car_number: &str,
    ) -> Result<u32, BroadcastError>;
}

#[async_trait]
pub(crate) trait ReplayStatePort: Send + Sync {
    async fn speed_snapshot(&self) -> Result<ReplaySpeedSnapshot, BroadcastError>;

    async fn wait_for_speed(
        &self,
        previous: ReplaySpeedSnapshot,
        expected: ReplaySpeedExpectation,
        timeout: Duration,
    ) -> Result<ReplaySpeedSnapshot, BroadcastError>;

    async fn position_snapshot(&self) -> Result<ReplayPositionSnapshot, BroadcastError>;

    async fn wait_for_position(
        &self,
        previous: ReplayPositionSnapshot,
        expected: ReplayPositionExpectation,
        timeout: Duration,
    ) -> Result<ReplayPositionSnapshot, BroadcastError>;
}

#[async_trait]
pub(crate) trait PitStatePort: Send + Sync {
    async fn pit_service_snapshot(&self) -> Result<PitServiceSnapshot, BroadcastError>;

    async fn wait_for_pit_service(
        &self,
        previous: PitServiceSnapshot,
        expected: PitServiceExpectation,
        timeout: Duration,
    ) -> Result<PitServiceSnapshot, BroadcastError>;
}

#[async_trait]
pub(crate) trait TelemetryStatePort: Send + Sync {
    async fn logging_snapshot(&self) -> Result<TelemetryLoggingSnapshot, BroadcastError>;

    async fn wait_for_logging(
        &self,
        previous: TelemetryLoggingSnapshot,
        expected: TelemetryLoggingExpectation,
        timeout: Duration,
    ) -> Result<TelemetryLoggingSnapshot, BroadcastError>;
}

#[async_trait]
pub(crate) trait ForceFeedbackStatePort: Send + Sync {
    async fn force_feedback_snapshot(&self) -> Result<ForceFeedbackSnapshot, BroadcastError>;

    async fn wait_for_force_feedback(
        &self,
        previous: ForceFeedbackSnapshot,
        expected: ForceFeedbackExpectation,
        timeout: Duration,
    ) -> Result<ForceFeedbackSnapshot, BroadcastError>;
}

#[derive(Debug, Default)]
pub(crate) struct DisabledObservationPort;

#[async_trait]
impl CameraStatePort for DisabledObservationPort {
    async fn selection_snapshot(&self) -> Result<CameraSelectionSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }

    async fn wait_for_selection(
        &self,
        _previous: CameraSelectionSnapshot,
        _expected: CameraSelectionExpectation,
        _timeout: Duration,
    ) -> Result<CameraSelectionSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }

    async fn state_snapshot(&self) -> Result<CameraStateSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }

    async fn wait_for_state(
        &self,
        _previous: CameraStateSnapshot,
        _expected: CameraStateExpectation,
        _timeout: Duration,
    ) -> Result<CameraStateSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }

    async fn available_camera_groups(
        &self,
        _session_version: u32,
    ) -> Result<Vec<AvailableCameraGroup>, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }

    async fn resolve_car_index_by_number(
        &self,
        _session_version: u32,
        _car_number: &str,
    ) -> Result<u32, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }
}

#[async_trait]
impl ReplayStatePort for DisabledObservationPort {
    async fn speed_snapshot(&self) -> Result<ReplaySpeedSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }

    async fn wait_for_speed(
        &self,
        _previous: ReplaySpeedSnapshot,
        _expected: ReplaySpeedExpectation,
        _timeout: Duration,
    ) -> Result<ReplaySpeedSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }

    async fn position_snapshot(&self) -> Result<ReplayPositionSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }

    async fn wait_for_position(
        &self,
        _previous: ReplayPositionSnapshot,
        _expected: ReplayPositionExpectation,
        _timeout: Duration,
    ) -> Result<ReplayPositionSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }
}

#[async_trait]
impl PitStatePort for DisabledObservationPort {
    async fn pit_service_snapshot(&self) -> Result<PitServiceSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }

    async fn wait_for_pit_service(
        &self,
        _previous: PitServiceSnapshot,
        _expected: PitServiceExpectation,
        _timeout: Duration,
    ) -> Result<PitServiceSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }
}

#[async_trait]
impl TelemetryStatePort for DisabledObservationPort {
    async fn logging_snapshot(&self) -> Result<TelemetryLoggingSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }

    async fn wait_for_logging(
        &self,
        _previous: TelemetryLoggingSnapshot,
        _expected: TelemetryLoggingExpectation,
        _timeout: Duration,
    ) -> Result<TelemetryLoggingSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }
}

#[async_trait]
impl ForceFeedbackStatePort for DisabledObservationPort {
    async fn force_feedback_snapshot(&self) -> Result<ForceFeedbackSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }

    async fn wait_for_force_feedback(
        &self,
        _previous: ForceFeedbackSnapshot,
        _expected: ForceFeedbackExpectation,
        _timeout: Duration,
    ) -> Result<ForceFeedbackSnapshot, BroadcastError> {
        Err(BroadcastError::ObservationDisabled)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn dummy_camera_selection() -> CameraSelectionSnapshot {
        CameraSelectionSnapshot {
            session_version: 1,
            car_index: 0,
            group: 0,
            camera: 0,
        }
    }

    fn dummy_camera_state() -> CameraStateSnapshot {
        CameraStateSnapshot { state: 0 }
    }

    fn dummy_replay_speed() -> ReplaySpeedSnapshot {
        ReplaySpeedSnapshot {
            speed: 0,
            is_slow_motion: false,
        }
    }

    fn dummy_replay_position() -> ReplayPositionSnapshot {
        ReplayPositionSnapshot {
            frame: 0,
            session_number: 0,
            session_time: 0.0,
        }
    }

    fn dummy_pit_service() -> PitServiceSnapshot {
        PitServiceSnapshot {
            service_flags: 0,
            fuel: 0.0,
            lf_pressure: 0.0,
            rf_pressure: 0.0,
            lr_pressure: 0.0,
            rr_pressure: 0.0,
            tire_compound: 0,
        }
    }

    fn dummy_telemetry_logging() -> TelemetryLoggingSnapshot {
        TelemetryLoggingSnapshot {
            is_disk_logging_enabled: false,
            is_disk_logging_active: false,
        }
    }

    fn dummy_force_feedback() -> ForceFeedbackSnapshot {
        ForceFeedbackSnapshot { max_force: 0.0 }
    }

    #[tokio::test]
    async fn camera_state_port_always_returns_observation_disabled() {
        let port = DisabledObservationPort;
        let timeout = Duration::from_millis(10);

        assert!(matches!(
            port.selection_snapshot().await,
            Err(BroadcastError::ObservationDisabled)
        ));
        assert!(matches!(
            port.wait_for_selection(
                dummy_camera_selection(),
                CameraSelectionExpectation {
                    car_index: None,
                    group: 0,
                    camera: 0,
                },
                timeout,
            )
            .await,
            Err(BroadcastError::ObservationDisabled)
        ));
        assert!(matches!(
            port.state_snapshot().await,
            Err(BroadcastError::ObservationDisabled)
        ));
        assert!(matches!(
            port.wait_for_state(
                dummy_camera_state(),
                CameraStateExpectation { state: 0 },
                timeout,
            )
            .await,
            Err(BroadcastError::ObservationDisabled)
        ));
        assert!(matches!(
            port.available_camera_groups(1).await,
            Err(BroadcastError::ObservationDisabled)
        ));
        assert!(matches!(
            port.resolve_car_index_by_number(1, "012").await,
            Err(BroadcastError::ObservationDisabled)
        ));
    }

    #[tokio::test]
    async fn replay_state_port_always_returns_observation_disabled() {
        let port = DisabledObservationPort;
        let timeout = Duration::from_millis(10);

        assert!(matches!(
            port.speed_snapshot().await,
            Err(BroadcastError::ObservationDisabled)
        ));
        assert!(matches!(
            port.wait_for_speed(
                dummy_replay_speed(),
                ReplaySpeedExpectation {
                    speed: 1,
                    is_slow_motion: false,
                },
                timeout,
            )
            .await,
            Err(BroadcastError::ObservationDisabled)
        ));
        assert!(matches!(
            port.position_snapshot().await,
            Err(BroadcastError::ObservationDisabled)
        ));
        assert!(matches!(
            port.wait_for_position(
                dummy_replay_position(),
                ReplayPositionExpectation::AnyChange,
                timeout,
            )
            .await,
            Err(BroadcastError::ObservationDisabled)
        ));
    }

    #[tokio::test]
    async fn pit_state_port_always_returns_observation_disabled() {
        let port = DisabledObservationPort;
        let timeout = Duration::from_millis(10);

        assert!(matches!(
            port.pit_service_snapshot().await,
            Err(BroadcastError::ObservationDisabled)
        ));
        assert!(matches!(
            port.wait_for_pit_service(
                dummy_pit_service(),
                PitServiceExpectation::AnyChange,
                timeout,
            )
            .await,
            Err(BroadcastError::ObservationDisabled)
        ));
    }

    #[tokio::test]
    async fn telemetry_state_port_always_returns_observation_disabled() {
        let port = DisabledObservationPort;
        let timeout = Duration::from_millis(10);

        assert!(matches!(
            port.logging_snapshot().await,
            Err(BroadcastError::ObservationDisabled)
        ));
        assert!(matches!(
            port.wait_for_logging(
                dummy_telemetry_logging(),
                TelemetryLoggingExpectation {
                    is_disk_logging_enabled: true,
                },
                timeout,
            )
            .await,
            Err(BroadcastError::ObservationDisabled)
        ));
    }

    #[tokio::test]
    async fn force_feedback_port_always_returns_observation_disabled() {
        let port = DisabledObservationPort;
        let timeout = Duration::from_millis(10);

        assert!(matches!(
            port.force_feedback_snapshot().await,
            Err(BroadcastError::ObservationDisabled)
        ));
        assert!(matches!(
            port.wait_for_force_feedback(
                dummy_force_feedback(),
                ForceFeedbackExpectation { max_force: 10.0 },
                timeout,
            )
            .await,
            Err(BroadcastError::ObservationDisabled)
        ));
    }

    #[test]
    fn disabled_observation_port_is_default_constructible() {
        let _port: DisabledObservationPort = DisabledObservationPort::default();
    }
}
