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

    async fn resolve_car_number_by_index(
        &self,
        session_version: u32,
        car_index: u32,
    ) -> Result<String, BroadcastError>;
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

    async fn resolve_car_number_by_index(
        &self,
        _session_version: u32,
        _car_index: u32,
    ) -> Result<String, BroadcastError> {
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
