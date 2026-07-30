use std::{
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};

use async_trait::async_trait;
use iracing_sdk::{
    CameraState, FrameAdapter, VariableSchema,
    provider::Provider,
    providers::live::LiveProvider,
    schema::{SessionInfo, SessionInfoParser},
};
use tokio::sync::Mutex;

use crate::{
    broadcast_app::{
        AvailableCamera, AvailableCameraGroup, BroadcastError, CameraSelectionExpectation,
        CameraSelectionSnapshot, CameraStateExpectation, CameraStatePort, CameraStateSnapshot,
        ForceFeedbackExpectation, ForceFeedbackSnapshot, ForceFeedbackStatePort,
        PitServiceExpectation, PitServiceSnapshot, PitStatePort, ReplayPositionExpectation,
        ReplayPositionSnapshot, ReplaySpeedExpectation, ReplaySpeedSnapshot, ReplayStatePort,
        TelemetryLoggingExpectation, TelemetryLoggingSnapshot, TelemetryStatePort,
    },
    telemetry_observer::{
        CameraSelectionTelemetry, CameraStateTelemetry, ForceFeedbackTelemetry, ObservedValue,
        PitServiceTelemetry, ReplayPositionTelemetry, ReplaySpeedTelemetry,
        TelemetryLoggingTelemetry, TelemetryObserver, TelemetryObserverError,
    },
};

#[derive(Debug, Clone)]
struct CachedSessionInfo {
    version: u32,
    session: SessionInfo,
}

pub(crate) struct IracingObservation<P> {
    provider: Arc<Mutex<P>>,
    telemetry: TelemetryObserver<P>,
    session_parser: Arc<StdMutex<SessionInfoParser>>,
    session_cache: Arc<StdMutex<Option<CachedSessionInfo>>>,
    camera_selection_available: bool,
    camera_state_available: bool,
    replay_speed_available: bool,
    replay_position_available: bool,
    pit_service_available: bool,
    telemetry_logging_available: bool,
    force_feedback_available: bool,
}

impl IracingObservation<LiveProvider> {
    pub(crate) fn live() -> iracing_sdk::Result<Self> {
        let provider = LiveProvider::new()?;
        let schema = provider.schema();
        Ok(Self::from_provider(provider, schema))
    }
}

impl<P> IracingObservation<P>
where
    P: Provider,
{
    pub(crate) fn from_provider(provider: P, schema: Arc<VariableSchema>) -> Self {
        let provider = Arc::new(Mutex::new(provider));
        let telemetry = TelemetryObserver::new(Arc::clone(&provider), schema);

        Self {
            provider,
            camera_selection_available: Self::validate_capability::<CameraSelectionTelemetry>(
                &telemetry,
                "camera selection",
            ),
            camera_state_available: Self::validate_capability::<CameraStateTelemetry>(
                &telemetry,
                "camera state",
            ),
            replay_speed_available: Self::validate_capability::<ReplaySpeedTelemetry>(
                &telemetry,
                "replay speed",
            ),
            replay_position_available: Self::validate_capability::<ReplayPositionTelemetry>(
                &telemetry,
                "replay position",
            ),
            pit_service_available: Self::validate_capability::<PitServiceTelemetry>(
                &telemetry,
                "pit service",
            ),
            telemetry_logging_available: Self::validate_capability::<TelemetryLoggingTelemetry>(
                &telemetry,
                "telemetry logging",
            ),
            force_feedback_available: Self::validate_capability::<ForceFeedbackTelemetry>(
                &telemetry,
                "force feedback",
            ),
            telemetry,
            session_parser: Arc::new(StdMutex::new(SessionInfoParser::new())),
            session_cache: Arc::new(StdMutex::new(None)),
        }
    }

    fn validate_capability<A>(telemetry: &TelemetryObserver<P>, name: &'static str) -> bool
    where
        A: FrameAdapter,
    {
        match telemetry.validate::<A>() {
            Ok(()) => true,
            Err(error) => {
                tracing::debug!(%error, capability = name, "broadcast observation capability unavailable");
                false
            }
        }
    }

    fn require_capability(
        &self,
        enabled: bool,
        capability: &'static str,
    ) -> Result<(), BroadcastError> {
        if enabled {
            Ok(())
        } else {
            Err(BroadcastError::CapabilityUnavailable(capability))
        }
    }

    fn map_telemetry_error(error: TelemetryObserverError) -> BroadcastError {
        match error {
            TelemetryObserverError::Timeout => BroadcastError::ObservationTimeout,
            TelemetryObserverError::EndOfSource => BroadcastError::ObservationSourceEnded,
            TelemetryObserverError::Sdk(error) => BroadcastError::Sdk(error),
        }
    }

    async fn session_info(&self, version: u32) -> Result<SessionInfo, BroadcastError> {
        if let Some(cached) = self
            .session_cache
            .lock()
            .expect("session cache mutex poisoned")
            .clone()
            .filter(|cached| cached.version == version)
        {
            return Ok(cached.session);
        }

        let yaml = {
            let mut provider = self.provider.lock().await;
            Provider::session_yaml(&mut *provider, version).await?
        };

        let yaml = yaml.ok_or_else(|| {
            BroadcastError::FailedPrecondition(format!(
                "session data is unavailable for telemetry version {version}"
            ))
        })?;

        let session = self
            .session_parser
            .lock()
            .expect("session parser mutex poisoned")
            .parse(&yaml)
            .map_err(|error| BroadcastError::FailedPrecondition(error.to_string()))?;

        *self
            .session_cache
            .lock()
            .expect("session cache mutex poisoned") = Some(CachedSessionInfo {
            version,
            session: session.clone(),
        });

        Ok(session)
    }
}

const CONTROLLABLE_CAMERA_STATE: CameraState = CameraState::CAM_TOOL_ACTIVE
    .union(CameraState::UI_HIDDEN)
    .union(CameraState::USE_AUTO_SHOT_SELECTION)
    .union(CameraState::USE_TEMPORARY_EDITS)
    .union(CameraState::USE_KEY_ACCELERATION)
    .union(CameraState::USE_KEY_10X_ACCELERATION)
    .union(CameraState::USE_MOUSE_AIM_MODE);

fn camera_state_matches_expectation(
    current: CameraState,
    expected: CameraStateExpectation,
) -> bool {
    let expected = CameraState::from_bits_retain(expected.state);

    if expected.bits() == 0 {
        !current.intersects(CONTROLLABLE_CAMERA_STATE)
    } else {
        current.contains(expected)
    }
}

#[async_trait]
impl<P> CameraStatePort for IracingObservation<P>
where
    P: Provider + Sync,
{
    async fn selection_snapshot(&self) -> Result<CameraSelectionSnapshot, BroadcastError> {
        self.require_capability(self.camera_selection_available, "camera selection")?;
        let observed = self
            .telemetry
            .snapshot_observed::<CameraSelectionTelemetry>()
            .await
            .map_err(Self::map_telemetry_error)?;

        observed.try_into()
    }

    async fn wait_for_selection(
        &self,
        previous: CameraSelectionSnapshot,
        expected: CameraSelectionExpectation,
        timeout: Duration,
    ) -> Result<CameraSelectionSnapshot, BroadcastError> {
        self.require_capability(self.camera_selection_available, "camera selection")?;
        let previous = camera_snapshot_to_telemetry(previous)?;
        let observed = self
            .telemetry
            .wait_for_change_matching_observed(previous, timeout, move |current| {
                let current_car_index = u32::try_from(current.car_index).ok();
                let current_group = u32::try_from(current.group).ok();
                let current_camera = u32::try_from(current.camera).ok();

                current_group == Some(expected.group)
                    && current_camera == Some(expected.camera)
                    && expected
                        .car_index
                        .is_none_or(|car_index| current_car_index == Some(car_index))
            })
            .await
            .map_err(Self::map_telemetry_error)?;

        observed.try_into()
    }

    async fn state_snapshot(&self) -> Result<CameraStateSnapshot, BroadcastError> {
        self.require_capability(self.camera_state_available, "camera state")?;
        let observed = self
            .telemetry
            .snapshot_observed::<CameraStateTelemetry>()
            .await
            .map_err(Self::map_telemetry_error)?;

        Ok(observed.into())
    }

    async fn wait_for_state(
        &self,
        previous: CameraStateSnapshot,
        expected: CameraStateExpectation,
        timeout: Duration,
    ) -> Result<CameraStateSnapshot, BroadcastError> {
        self.require_capability(self.camera_state_available, "camera state")?;
        let previous = CameraStateTelemetry {
            state: CameraState::from_bits_retain(previous.state),
        };

        if camera_state_matches_expectation(previous.state, expected) {
            return Ok(CameraStateSnapshot {
                state: previous.state.bits(),
            });
        }

        let observed = self
            .telemetry
            .wait_for_change_matching_observed(previous, timeout, move |current| {
                camera_state_matches_expectation(current.state, expected)
            })
            .await
            .map_err(Self::map_telemetry_error)?;

        Ok(observed.into())
    }

    async fn available_camera_groups(
        &self,
        session_version: u32,
    ) -> Result<Vec<AvailableCameraGroup>, BroadcastError> {
        self.require_capability(self.camera_selection_available, "camera selection")?;

        let session = self.session_info(session_version).await?;
        let groups = session
            .camera_info
            .and_then(|camera_info| camera_info.groups)
            .ok_or_else(|| {
                BroadcastError::FailedPrecondition(
                    "session camera groups are unavailable".to_string(),
                )
            })?;

        Ok(groups
            .into_iter()
            .filter_map(|group| {
                let number = group
                    .group_num
                    .and_then(|value| u32::try_from(value).ok())?;
                Some(AvailableCameraGroup {
                    number,
                    name: group.group_name.unwrap_or_default(),
                    cameras: group
                        .cameras
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|camera| {
                            let number = camera
                                .camera_num
                                .and_then(|value| u32::try_from(value).ok())?;
                            Some(AvailableCamera {
                                number,
                                name: camera.camera_name,
                            })
                        })
                        .collect(),
                })
            })
            .collect())
    }

    async fn resolve_car_index_by_number(
        &self,
        session_version: u32,
        car_number: &str,
    ) -> Result<u32, BroadcastError> {
        let session = self.session_info(session_version).await?;
        let drivers = session
            .driver_info
            .and_then(|driver_info| driver_info.drivers)
            .ok_or_else(|| {
                BroadcastError::FailedPrecondition("session driver list is unavailable".to_string())
            })?;

        let driver = drivers
            .into_iter()
            .find(|driver| driver.car_number.as_deref() == Some(car_number))
            .ok_or_else(|| {
                BroadcastError::FailedPrecondition(format!(
                    "car number `{car_number}` was not found in session driver info"
                ))
            })?;

        u32::try_from(driver.car_idx).map_err(|_| {
            BroadcastError::FailedPrecondition(format!(
                "car number `{car_number}` resolved to invalid car index {}",
                driver.car_idx
            ))
        })
    }

    async fn resolve_car_number_by_index(
        &self,
        session_version: u32,
        car_index: u32,
    ) -> Result<String, BroadcastError> {
        let session = self.session_info(session_version).await?;
        let drivers = session
            .driver_info
            .and_then(|driver_info| driver_info.drivers)
            .ok_or_else(|| {
                BroadcastError::FailedPrecondition("session driver list is unavailable".to_string())
            })?;

        let driver = drivers
            .into_iter()
            .find(|driver| u32::try_from(driver.car_idx).ok() == Some(car_index))
            .ok_or_else(|| {
                BroadcastError::FailedPrecondition(format!(
                    "car index `{car_index}` was not found in session driver info"
                ))
            })?;

        driver.car_number.ok_or_else(|| {
            BroadcastError::FailedPrecondition(format!(
                "car index `{car_index}` does not have a car number in session driver info"
            ))
        })
    }
}

#[async_trait]
impl<P> ReplayStatePort for IracingObservation<P>
where
    P: Provider + Sync,
{
    async fn speed_snapshot(&self) -> Result<ReplaySpeedSnapshot, BroadcastError> {
        self.require_capability(self.replay_speed_available, "replay speed")?;
        let observed = self
            .telemetry
            .snapshot_observed::<ReplaySpeedTelemetry>()
            .await
            .map_err(Self::map_telemetry_error)?;

        Ok(observed.into())
    }

    async fn wait_for_speed(
        &self,
        previous: ReplaySpeedSnapshot,
        expected: ReplaySpeedExpectation,
        timeout: Duration,
    ) -> Result<ReplaySpeedSnapshot, BroadcastError> {
        self.require_capability(self.replay_speed_available, "replay speed")?;
        let previous = ReplaySpeedTelemetry {
            speed: previous.speed,
            is_slow_motion: previous.is_slow_motion,
        };
        let observed = self
            .telemetry
            .wait_for_change_matching_observed(previous, timeout, move |current| {
                current.speed == expected.speed && current.is_slow_motion == expected.is_slow_motion
            })
            .await
            .map_err(Self::map_telemetry_error)?;

        Ok(observed.into())
    }

    async fn position_snapshot(&self) -> Result<ReplayPositionSnapshot, BroadcastError> {
        self.require_capability(self.replay_position_available, "replay position")?;
        let observed = self
            .telemetry
            .snapshot_observed::<ReplayPositionTelemetry>()
            .await
            .map_err(Self::map_telemetry_error)?;

        observed.try_into()
    }

    async fn wait_for_position(
        &self,
        previous: ReplayPositionSnapshot,
        expected: ReplayPositionExpectation,
        timeout: Duration,
    ) -> Result<ReplayPositionSnapshot, BroadcastError> {
        self.require_capability(self.replay_position_available, "replay position")?;
        let previous = replay_position_snapshot_to_telemetry(previous)?;
        let observed = self
            .telemetry
            .wait_for_change_matching_observed(previous, timeout, move |_current| {
                matches!(expected, ReplayPositionExpectation::AnyChange)
            })
            .await
            .map_err(Self::map_telemetry_error)?;

        observed.try_into()
    }
}

#[async_trait]
impl<P> PitStatePort for IracingObservation<P>
where
    P: Provider + Sync,
{
    async fn pit_service_snapshot(&self) -> Result<PitServiceSnapshot, BroadcastError> {
        self.require_capability(self.pit_service_available, "pit service")?;
        let observed = self
            .telemetry
            .snapshot_observed::<PitServiceTelemetry>()
            .await
            .map_err(Self::map_telemetry_error)?;

        observed.try_into()
    }

    async fn wait_for_pit_service(
        &self,
        previous: PitServiceSnapshot,
        expected: PitServiceExpectation,
        timeout: Duration,
    ) -> Result<PitServiceSnapshot, BroadcastError> {
        self.require_capability(self.pit_service_available, "pit service")?;
        let previous = pit_service_snapshot_to_telemetry(previous)?;
        let observed = self
            .telemetry
            .wait_for_change_matching_observed(previous, timeout, move |_current| {
                matches!(expected, PitServiceExpectation::AnyChange)
            })
            .await
            .map_err(Self::map_telemetry_error)?;

        observed.try_into()
    }
}

#[async_trait]
impl<P> TelemetryStatePort for IracingObservation<P>
where
    P: Provider + Sync,
{
    async fn logging_snapshot(&self) -> Result<TelemetryLoggingSnapshot, BroadcastError> {
        self.require_capability(self.telemetry_logging_available, "telemetry logging")?;
        let observed = self
            .telemetry
            .snapshot_observed::<TelemetryLoggingTelemetry>()
            .await
            .map_err(Self::map_telemetry_error)?;

        Ok(observed.into())
    }

    async fn wait_for_logging(
        &self,
        previous: TelemetryLoggingSnapshot,
        expected: TelemetryLoggingExpectation,
        timeout: Duration,
    ) -> Result<TelemetryLoggingSnapshot, BroadcastError> {
        self.require_capability(self.telemetry_logging_available, "telemetry logging")?;
        let previous = TelemetryLoggingTelemetry {
            is_disk_logging_enabled: previous.is_disk_logging_enabled,
            is_disk_logging_active: previous.is_disk_logging_active,
        };
        let observed = self
            .telemetry
            .wait_for_change_matching_observed(previous, timeout, move |current| {
                current.is_disk_logging_enabled == expected.is_disk_logging_enabled
            })
            .await
            .map_err(Self::map_telemetry_error)?;

        Ok(observed.into())
    }
}

#[async_trait]
impl<P> ForceFeedbackStatePort for IracingObservation<P>
where
    P: Provider + Sync,
{
    async fn force_feedback_snapshot(&self) -> Result<ForceFeedbackSnapshot, BroadcastError> {
        self.require_capability(self.force_feedback_available, "force feedback")?;
        let observed = self
            .telemetry
            .snapshot_observed::<ForceFeedbackTelemetry>()
            .await
            .map_err(Self::map_telemetry_error)?;

        Ok(observed.into())
    }

    async fn wait_for_force_feedback(
        &self,
        previous: ForceFeedbackSnapshot,
        expected: ForceFeedbackExpectation,
        timeout: Duration,
    ) -> Result<ForceFeedbackSnapshot, BroadcastError> {
        self.require_capability(self.force_feedback_available, "force feedback")?;
        let previous = ForceFeedbackTelemetry {
            max_force: previous.max_force,
        };
        let observed = self
            .telemetry
            .wait_for_change_matching_observed(previous, timeout, move |current| {
                (current.max_force - expected.max_force).abs() <= 0.000_1
            })
            .await
            .map_err(Self::map_telemetry_error)?;

        Ok(observed.into())
    }
}

impl TryFrom<ObservedValue<CameraSelectionTelemetry>> for CameraSelectionSnapshot {
    type Error = BroadcastError;

    fn try_from(observed: ObservedValue<CameraSelectionTelemetry>) -> Result<Self, Self::Error> {
        Ok(Self {
            session_version: observed.session_version,
            car_index: non_negative_u32("car_index", observed.value.car_index)?,
            group: non_negative_u32("group", observed.value.group)?,
            camera: non_negative_u32("camera", observed.value.camera)?,
        })
    }
}

impl From<ObservedValue<ReplaySpeedTelemetry>> for ReplaySpeedSnapshot {
    fn from(observed: ObservedValue<ReplaySpeedTelemetry>) -> Self {
        Self {
            speed: observed.value.speed,
            is_slow_motion: observed.value.is_slow_motion,
        }
    }
}

impl From<ObservedValue<CameraStateTelemetry>> for CameraStateSnapshot {
    fn from(observed: ObservedValue<CameraStateTelemetry>) -> Self {
        Self {
            state: observed.value.state.bits(),
        }
    }
}

impl TryFrom<ObservedValue<ReplayPositionTelemetry>> for ReplayPositionSnapshot {
    type Error = BroadcastError;

    fn try_from(observed: ObservedValue<ReplayPositionTelemetry>) -> Result<Self, Self::Error> {
        Ok(Self {
            frame: non_negative_u32("frame", observed.value.frame)?,
            session_number: non_negative_u32("session_number", observed.value.session_number)?,
            session_time: non_negative_f32_from_f64("session_time", observed.value.session_time)?,
        })
    }
}

impl TryFrom<ObservedValue<PitServiceTelemetry>> for PitServiceSnapshot {
    type Error = BroadcastError;

    fn try_from(observed: ObservedValue<PitServiceTelemetry>) -> Result<Self, Self::Error> {
        Ok(Self {
            service_flags: observed.value.service_flags.bits(),
            fuel: finite_f32("fuel", observed.value.fuel)?,
            lf_pressure: finite_f32("lf_pressure", observed.value.lf_pressure)?,
            rf_pressure: finite_f32("rf_pressure", observed.value.rf_pressure)?,
            lr_pressure: finite_f32("lr_pressure", observed.value.lr_pressure)?,
            rr_pressure: finite_f32("rr_pressure", observed.value.rr_pressure)?,
            tire_compound: non_negative_u32("tire_compound", observed.value.tire_compound)?,
        })
    }
}

impl From<ObservedValue<TelemetryLoggingTelemetry>> for TelemetryLoggingSnapshot {
    fn from(observed: ObservedValue<TelemetryLoggingTelemetry>) -> Self {
        Self {
            is_disk_logging_enabled: observed.value.is_disk_logging_enabled,
            is_disk_logging_active: observed.value.is_disk_logging_active,
        }
    }
}

impl From<ObservedValue<ForceFeedbackTelemetry>> for ForceFeedbackSnapshot {
    fn from(observed: ObservedValue<ForceFeedbackTelemetry>) -> Self {
        Self {
            max_force: observed.value.max_force,
        }
    }
}

fn camera_snapshot_to_telemetry(
    snapshot: CameraSelectionSnapshot,
) -> Result<CameraSelectionTelemetry, BroadcastError> {
    Ok(CameraSelectionTelemetry {
        car_index: i32::try_from(snapshot.car_index).map_err(|_| {
            BroadcastError::FailedPrecondition(format!(
                "observed `car_index` must fit in i32, got {}",
                snapshot.car_index
            ))
        })?,
        group: i32::try_from(snapshot.group).map_err(|_| {
            BroadcastError::FailedPrecondition(format!(
                "observed `group` must fit in i32, got {}",
                snapshot.group
            ))
        })?,
        camera: i32::try_from(snapshot.camera).map_err(|_| {
            BroadcastError::FailedPrecondition(format!(
                "observed `camera` must fit in i32, got {}",
                snapshot.camera
            ))
        })?,
    })
}

fn replay_position_snapshot_to_telemetry(
    snapshot: ReplayPositionSnapshot,
) -> Result<ReplayPositionTelemetry, BroadcastError> {
    Ok(ReplayPositionTelemetry {
        frame: i32::try_from(snapshot.frame).map_err(|_| {
            BroadcastError::FailedPrecondition(format!(
                "observed `frame` must fit in i32, got {}",
                snapshot.frame
            ))
        })?,
        session_number: i32::try_from(snapshot.session_number).map_err(|_| {
            BroadcastError::FailedPrecondition(format!(
                "observed `session_number` must fit in i32, got {}",
                snapshot.session_number
            ))
        })?,
        session_time: f64::from(snapshot.session_time),
    })
}

fn pit_service_snapshot_to_telemetry(
    snapshot: PitServiceSnapshot,
) -> Result<PitServiceTelemetry, BroadcastError> {
    Ok(PitServiceTelemetry {
        service_flags: iracing_sdk::PitServiceFlags::from_bits_retain(snapshot.service_flags),
        fuel: snapshot.fuel,
        lf_pressure: snapshot.lf_pressure,
        rf_pressure: snapshot.rf_pressure,
        lr_pressure: snapshot.lr_pressure,
        rr_pressure: snapshot.rr_pressure,
        tire_compound: i32::try_from(snapshot.tire_compound).map_err(|_| {
            BroadcastError::FailedPrecondition(format!(
                "observed `tire_compound` must fit in i32, got {}",
                snapshot.tire_compound
            ))
        })?,
    })
}

fn non_negative_u32(field_name: &'static str, value: i32) -> Result<u32, BroadcastError> {
    u32::try_from(value).map_err(|_| {
        BroadcastError::FailedPrecondition(format!(
            "observed `{field_name}` must be non-negative, got {value}"
        ))
    })
}

fn non_negative_f32_from_f64(field_name: &'static str, value: f64) -> Result<f32, BroadcastError> {
    if value.is_finite() && value >= 0.0 && value <= f64::from(f32::MAX) {
        Ok(value as f32)
    } else {
        Err(BroadcastError::FailedPrecondition(format!(
            "observed `{field_name}` must be a finite non-negative f32-compatible value, got {value}"
        )))
    }
}

fn finite_f32(field_name: &'static str, value: f32) -> Result<f32, BroadcastError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(BroadcastError::FailedPrecondition(format!(
            "observed `{field_name}` must be finite, got {value}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        sync::Arc,
        time::Duration,
    };

    use async_trait::async_trait;
    use iracing_sdk::{CameraState, FramePacket, IRacingSDKError, VariableInfo, VariableType};

    use super::*;

    struct FakeProvider {
        frames: VecDeque<Result<Option<FramePacket>, IRacingSDKError>>,
    }

    impl FakeProvider {
        fn new(frames: impl IntoIterator<Item = FramePacket>) -> Self {
            Self {
                frames: frames.into_iter().map(|frame| Ok(Some(frame))).collect(),
            }
        }
    }

    #[async_trait]
    impl Provider for FakeProvider {
        async fn next_frame(&mut self) -> iracing_sdk::Result<Option<FramePacket>> {
            self.frames.pop_front().unwrap_or(Ok(None))
        }

        async fn session_yaml(&mut self, _version: u32) -> iracing_sdk::Result<Option<String>> {
            Ok(None)
        }

        fn tick_rate(&self) -> f64 {
            60.0
        }
    }

    fn observed_camera(
        session_version: u32,
        car_index: i32,
        group: i32,
        camera: i32,
    ) -> ObservedValue<CameraSelectionTelemetry> {
        ObservedValue {
            value: CameraSelectionTelemetry {
                car_index,
                group,
                camera,
            },
            session_version,
        }
    }

    fn observed_replay(
        session_version: u32,
        speed: i32,
        is_slow_motion: bool,
    ) -> ObservedValue<ReplaySpeedTelemetry> {
        ObservedValue {
            value: ReplaySpeedTelemetry {
                speed,
                is_slow_motion,
            },
            session_version,
        }
    }

    fn camera_state_schema() -> Arc<VariableSchema> {
        let variables = HashMap::from([(
            "CamCameraState".to_string(),
            VariableInfo {
                name: "CamCameraState".to_string(),
                data_type: VariableType::BitField,
                offset: 0,
                count: 1,
                count_as_time: false,
                units: String::new(),
                description: String::new(),
            },
        )]);

        Arc::new(VariableSchema::new(variables, 4).expect("schema should be valid"))
    }

    fn camera_state_packet(
        schema: Arc<VariableSchema>,
        tick: u32,
        state: CameraState,
    ) -> FramePacket {
        let mut data = vec![0u8; 4];
        data.copy_from_slice(&state.bits().to_le_bytes());
        FramePacket::new(data, tick, 7, schema)
    }

    fn camera_state_snapshot(state: CameraState) -> CameraStateSnapshot {
        CameraStateSnapshot {
            state: state.bits(),
        }
    }

    fn camera_state_expectation(state: CameraState) -> CameraStateExpectation {
        CameraStateExpectation {
            state: state.bits(),
        }
    }

    #[test]
    fn camera_state_matcher_accepts_superset_flags() {
        let current = CameraState::IS_SESSION_SCREEN.union(CameraState::CAM_TOOL_ACTIVE);

        assert!(camera_state_matches_expectation(
            current,
            camera_state_expectation(CameraState::CAM_TOOL_ACTIVE),
        ));
    }

    #[test]
    fn camera_state_matcher_requires_all_expected_flags() {
        let expected = CameraState::CAM_TOOL_ACTIVE.union(CameraState::UI_HIDDEN);

        assert!(!camera_state_matches_expectation(
            CameraState::CAM_TOOL_ACTIVE,
            camera_state_expectation(expected),
        ));
        assert!(camera_state_matches_expectation(
            expected.union(CameraState::IS_SESSION_SCREEN),
            camera_state_expectation(expected),
        ));
    }

    #[test]
    fn camera_state_matcher_treats_zero_as_no_controllable_flags() {
        assert!(camera_state_matches_expectation(
            CameraState::IS_SESSION_SCREEN.union(CameraState::IS_SCENIC_ACTIVE),
            camera_state_expectation(CameraState::empty()),
        ));
        assert!(!camera_state_matches_expectation(
            CameraState::IS_SESSION_SCREEN.union(CameraState::UI_HIDDEN),
            camera_state_expectation(CameraState::empty()),
        ));
    }

    #[tokio::test]
    async fn camera_state_wait_returns_previous_when_already_satisfied() {
        let observation =
            IracingObservation::from_provider(FakeProvider::new([]), camera_state_schema());
        let previous = CameraState::IS_SESSION_SCREEN.union(CameraState::CAM_TOOL_ACTIVE);

        let actual = observation
            .wait_for_state(
                camera_state_snapshot(previous),
                camera_state_expectation(CameraState::CAM_TOOL_ACTIVE),
                Duration::from_millis(10),
            )
            .await
            .expect("already-satisfied state should return immediately");

        assert_eq!(actual, camera_state_snapshot(previous));
    }

    #[tokio::test]
    async fn camera_state_wait_accepts_changed_superset_state() {
        let schema = camera_state_schema();
        let observed = CameraState::IS_SESSION_SCREEN.union(CameraState::CAM_TOOL_ACTIVE);
        let observation = IracingObservation::from_provider(
            FakeProvider::new([camera_state_packet(Arc::clone(&schema), 1, observed)]),
            schema,
        );

        let actual = observation
            .wait_for_state(
                camera_state_snapshot(CameraState::empty()),
                camera_state_expectation(CameraState::CAM_TOOL_ACTIVE),
                Duration::from_millis(100),
            )
            .await
            .expect("changed superset state should satisfy expected flag");

        assert_eq!(actual, camera_state_snapshot(observed));
    }

    #[tokio::test]
    async fn camera_state_wait_accepts_zero_when_controllable_flags_clear() {
        let schema = camera_state_schema();
        let previous = CameraState::IS_SESSION_SCREEN.union(CameraState::UI_HIDDEN);
        let observed = CameraState::IS_SESSION_SCREEN;
        let observation = IracingObservation::from_provider(
            FakeProvider::new([camera_state_packet(Arc::clone(&schema), 1, observed)]),
            schema,
        );

        let actual = observation
            .wait_for_state(
                camera_state_snapshot(previous),
                camera_state_expectation(CameraState::empty()),
                Duration::from_millis(100),
            )
            .await
            .expect("zero state should allow read-only camera status bits");

        assert_eq!(actual, camera_state_snapshot(observed));
    }

    #[test]
    fn camera_snapshot_rejects_negative_observed_values() {
        let error = CameraSelectionSnapshot::try_from(observed_camera(7, -1, 2, 3))
            .expect_err("negative car index should fail");

        match error {
            BroadcastError::FailedPrecondition(message) => {
                assert!(message.contains("car_index"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn replay_speed_snapshot_preserves_observed_values() {
        let snapshot = ReplaySpeedSnapshot::from(observed_replay(9, -2, true));

        assert_eq!(
            snapshot,
            ReplaySpeedSnapshot {
                speed: -2,
                is_slow_motion: true,
            }
        );
    }
}
