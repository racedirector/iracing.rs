mod builder;
mod command;
mod error;
mod request;

use std::{
    future::Future,
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};

use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

use crate::broadcast::broadcast_server::Broadcast;
use crate::broadcast::*;
use crate::telemetry_observer::{
    CameraSelectionTelemetry, ObservedValue, ReplaySpeedTelemetry, TelemetryObserver,
    TelemetryObserverError,
};

use iracing_sdk::{
    Broadcast as BroadcastClient, BroadcastCommand, FrameAdapter, IRacingSDKError, LiveProvider,
    Provider, SessionInfo, SessionInfoParser, VariableSchema,
};

pub use builder::BroadcastServiceBuilder;
use command as command_impl;
use error::broadcast_error_to_status;
use request as request_impl;

const DEFAULT_OBSERVATION_TIMEOUT: Duration = Duration::from_secs(2);

trait BroadcastCommandSender: Send + Sync {
    fn send_message(&self, message: BroadcastCommand) -> Result<(), IRacingSDKError>;
}

impl BroadcastCommandSender for BroadcastClient {
    fn send_message(&self, message: BroadcastCommand) -> Result<(), IRacingSDKError> {
        BroadcastClient::send_message(self, message)
    }
}

#[derive(Debug, Clone, Copy)]
struct CameraSelectionExpectation {
    car_index: Option<u32>,
    group: u32,
    camera: u32,
}

#[derive(Debug, Clone, Copy)]
struct ReplaySpeedExpectation {
    speed: i32,
    is_slow_motion: bool,
}

trait ObservationBackend: Send + Sync {
    fn available_camera_groups(&self, session_version: u32) -> Result<Vec<CameraGroup>, Status>;
    fn camera_selection_snapshot(&self) -> Result<ObservedValue<CameraSelectionTelemetry>, Status>;
    fn wait_for_camera_selection(
        &self,
        previous: ObservedValue<CameraSelectionTelemetry>,
        expected: CameraSelectionExpectation,
        timeout: Duration,
    ) -> Result<ObservedValue<CameraSelectionTelemetry>, Status>;
    fn resolve_car_index_by_number(
        &self,
        session_version: u32,
        car_number: &str,
    ) -> Result<u32, Status>;
    fn replay_speed_snapshot(&self) -> Result<ObservedValue<ReplaySpeedTelemetry>, Status>;
    fn wait_for_replay_speed(
        &self,
        previous: ObservedValue<ReplaySpeedTelemetry>,
        expected: ReplaySpeedExpectation,
        timeout: Duration,
    ) -> Result<ObservedValue<ReplaySpeedTelemetry>, Status>;
}

#[derive(Debug, Clone)]
struct CachedSessionInfo {
    version: u32,
    session: SessionInfo,
}

struct ServiceObservation<P> {
    provider: Arc<Mutex<P>>,
    telemetry: TelemetryObserver<P>,
    session_parser: Arc<StdMutex<SessionInfoParser>>,
    session_cache: Arc<StdMutex<Option<CachedSessionInfo>>>,
    camera_selection_available: bool,
    replay_speed_available: bool,
}

impl ServiceObservation<LiveProvider> {
    fn live() -> Result<Self, IRacingSDKError> {
        let provider = LiveProvider::new()?;
        let schema = provider.schema();
        Ok(Self::from_provider(provider, schema))
    }
}

impl<P> ServiceObservation<P>
where
    P: Provider + Send + 'static,
{
    fn from_provider(provider: P, schema: Arc<VariableSchema>) -> Self {
        let provider = Arc::new(Mutex::new(provider));
        let telemetry = TelemetryObserver::new(Arc::clone(&provider), schema);

        Self {
            provider,
            camera_selection_available: Self::validate_capability::<CameraSelectionTelemetry>(
                &telemetry,
                "camera selection",
            ),
            replay_speed_available: Self::validate_capability::<ReplaySpeedTelemetry>(
                &telemetry,
                "replay speed",
            ),
            telemetry,
            session_parser: Arc::new(StdMutex::new(SessionInfoParser::new())),
            session_cache: Arc::new(StdMutex::new(None)),
        }
    }

    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
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

    fn require_capability(&self, enabled: bool, capability: &'static str) -> Result<(), Status> {
        if enabled {
            Ok(())
        } else {
            Err(Status::failed_precondition(format!(
                "{capability} telemetry is unavailable"
            )))
        }
    }

    fn map_telemetry_error(error: TelemetryObserverError) -> Status {
        match error {
            TelemetryObserverError::Timeout => {
                Status::deadline_exceeded("telemetry observation timed out")
            }
            TelemetryObserverError::EndOfSource => {
                Status::unavailable("telemetry source ended before the requested state change")
            }
            TelemetryObserverError::Sdk(error) => broadcast_error_to_status(error),
        }
    }

    fn session_info(&self, version: u32) -> Result<SessionInfo, Status> {
        if let Some(cached) = self
            .session_cache
            .lock()
            .expect("session cache mutex poisoned")
            .clone()
            .filter(|cached| cached.version == version)
        {
            return Ok(cached.session);
        }

        let yaml = self.block_on(async {
            let mut provider = self.provider.lock().await;
            provider
                .session_yaml(version)
                .await
                .map_err(broadcast_error_to_status)
        })?;

        let yaml = yaml.ok_or_else(|| {
            Status::failed_precondition(format!(
                "session data is unavailable for telemetry version {version}"
            ))
        })?;

        let session = self
            .session_parser
            .lock()
            .expect("session parser mutex poisoned")
            .parse(&yaml)
            .map_err(|error| Status::failed_precondition(error.to_string()))?;

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

impl<P> ObservationBackend for ServiceObservation<P>
where
    P: Provider + Send + 'static,
{
    fn available_camera_groups(&self, session_version: u32) -> Result<Vec<CameraGroup>, Status> {
        self.require_capability(self.camera_selection_available, "camera selection")?;

        let session = self.session_info(session_version)?;
        let groups = session
            .camera_info
            .and_then(|camera_info| camera_info.groups)
            .ok_or_else(|| Status::failed_precondition("session camera groups are unavailable"))?;

        Ok(groups
            .into_iter()
            .filter_map(|group| {
                let number = group
                    .group_num
                    .and_then(|value| u32::try_from(value).ok())?;
                Some(CameraGroup {
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
                            Some(CameraDetail {
                                number: Some(number),
                                name: camera.camera_name,
                            })
                        })
                        .collect(),
                })
            })
            .collect())
    }

    fn camera_selection_snapshot(&self) -> Result<ObservedValue<CameraSelectionTelemetry>, Status> {
        self.require_capability(self.camera_selection_available, "camera selection")?;
        self.block_on(
            self.telemetry
                .snapshot_observed::<CameraSelectionTelemetry>(),
        )
        .map_err(Self::map_telemetry_error)
    }

    fn wait_for_camera_selection(
        &self,
        previous: ObservedValue<CameraSelectionTelemetry>,
        expected: CameraSelectionExpectation,
        timeout: Duration,
    ) -> Result<ObservedValue<CameraSelectionTelemetry>, Status> {
        self.require_capability(self.camera_selection_available, "camera selection")?;
        self.block_on(self.telemetry.wait_for_change_matching_observed(
            previous.value,
            timeout,
            |current| {
                let current_car_index = u32::try_from(current.car_index).ok();
                let current_group = u32::try_from(current.group).ok();
                let current_camera = u32::try_from(current.camera).ok();

                current_group == Some(expected.group)
                    && current_camera == Some(expected.camera)
                    && expected
                        .car_index
                        .is_none_or(|car_index| current_car_index == Some(car_index))
            },
        ))
        .map_err(Self::map_telemetry_error)
    }

    fn resolve_car_index_by_number(
        &self,
        session_version: u32,
        car_number: &str,
    ) -> Result<u32, Status> {
        let session = self.session_info(session_version)?;
        let drivers = session
            .driver_info
            .and_then(|driver_info| driver_info.drivers)
            .ok_or_else(|| Status::failed_precondition("session driver list is unavailable"))?;

        let driver = drivers
            .into_iter()
            .find(|driver| driver.car_number.as_deref() == Some(car_number))
            .ok_or_else(|| {
                Status::failed_precondition(format!(
                    "car number `{car_number}` was not found in session driver info"
                ))
            })?;

        u32::try_from(driver.car_idx).map_err(|_| {
            Status::failed_precondition(format!(
                "car number `{car_number}` resolved to invalid car index {}",
                driver.car_idx
            ))
        })
    }

    fn replay_speed_snapshot(&self) -> Result<ObservedValue<ReplaySpeedTelemetry>, Status> {
        self.require_capability(self.replay_speed_available, "replay speed")?;
        self.block_on(self.telemetry.snapshot_observed::<ReplaySpeedTelemetry>())
            .map_err(Self::map_telemetry_error)
    }

    fn wait_for_replay_speed(
        &self,
        previous: ObservedValue<ReplaySpeedTelemetry>,
        expected: ReplaySpeedExpectation,
        timeout: Duration,
    ) -> Result<ObservedValue<ReplaySpeedTelemetry>, Status> {
        self.require_capability(self.replay_speed_available, "replay speed")?;
        self.block_on(self.telemetry.wait_for_change_matching_observed(
            previous.value,
            timeout,
            |current| {
                current.speed == expected.speed && current.is_slow_motion == expected.is_slow_motion
            },
        ))
        .map_err(Self::map_telemetry_error)
    }
}

pub struct BroadcastService {
    sender: Arc<dyn BroadcastCommandSender>,
    observation: Option<Arc<dyn ObservationBackend>>,
    observation_timeout: Duration,
}

impl BroadcastService {
    pub fn builder() -> crate::BroadcastServiceBuilder {
        crate::BroadcastServiceBuilder::default()
    }

    pub fn new() -> Result<Self, Status> {
        Self::builder().build().map_err(broadcast_error_to_status)
    }

    fn observation(&self) -> Result<&dyn ObservationBackend, Status> {
        self.observation.as_deref().ok_or_else(|| {
            Status::failed_precondition("broadcast service observation support is disabled")
        })
    }

    fn send_message(&self, message: BroadcastCommand) -> Result<(), Status> {
        self.sender
            .send_message(message)
            .map_err(broadcast_error_to_status)
    }

    async fn execute_ack<R>(
        &self,
        command: BroadcastCommand,
        response: R,
    ) -> Result<Response<R>, Status> {
        self.send_message(command)?;
        Ok(Response::new(response))
    }

    async fn execute_observed<Previous, Current, R>(
        &self,
        snapshot: impl FnOnce(&dyn ObservationBackend) -> Result<Previous, Status>,
        command: BroadcastCommand,
        wait: impl FnOnce(&dyn ObservationBackend, Previous, Duration) -> Result<Current, Status>,
        resolve: impl FnOnce(Current) -> Result<R, Status>,
    ) -> Result<Response<R>, Status> {
        let observation = self.observation()?;
        let previous = snapshot(observation)?;
        self.send_message(command)?;
        let current = wait(observation, previous, self.observation_timeout)?;
        Ok(Response::new(resolve(current)?))
    }
}

fn camera_switch_response(
    observed: ObservedValue<CameraSelectionTelemetry>,
) -> Result<CameraSwitchPositionResponse, Status> {
    Ok(CameraSwitchPositionResponse {
        car_index: non_negative_u32("car_index", observed.value.car_index)?,
        group: non_negative_u32("group", observed.value.group)?,
        camera: non_negative_u32("camera", observed.value.camera)?,
    })
}

fn replay_speed_response(
    observed: ObservedValue<ReplaySpeedTelemetry>,
) -> ReplaySetPlaySpeedResponse {
    ReplaySetPlaySpeedResponse {
        speed: observed.value.speed,
        is_slow_motion: observed.value.is_slow_motion,
    }
}

fn unsupported_state_resolution(operation: &'static str) -> Status {
    Status::failed_precondition(format!(
        "{operation} does not have validated telemetry-backed state resolution"
    ))
}

fn non_negative_u32(field_name: &'static str, value: i32) -> Result<u32, Status> {
    u32::try_from(value).map_err(|_| {
        Status::failed_precondition(format!(
            "observed `{field_name}` must be non-negative, got {value}"
        ))
    })
}

#[tonic::async_trait]
impl Broadcast for BroadcastService {
    #[tracing::instrument(
        name = "grpc.get_available_cameras",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "GetAvailableCameras",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn get_available_cameras(
        &self,
        request: Request<()>,
    ) -> Result<Response<GetAvailableCamerasResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let observation = self.observation()?;
        let current = observation.camera_selection_snapshot()?;
        let camera_groups = observation.available_camera_groups(current.session_version)?;

        Ok(Response::new(GetAvailableCamerasResponse {
            camera_groups,
            car_index: non_negative_u32("car_index", current.value.car_index)?,
            group: non_negative_u32("group", current.value.group)?,
            camera: non_negative_u32("camera", current.value.camera)?,
        }))
    }

    #[tracing::instrument(
        name = "grpc.camera_switch_position",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "CameraSwitchPosition",
            client.address = tracing::field::Empty,
            broadcast.position = tracing::field::Empty,
            broadcast.group = tracing::field::Empty,
            broadcast.camera = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn camera_switch_position(
        &self,
        request: Request<CameraSwitchPositionRequest>,
    ) -> Result<Response<CameraSwitchPositionResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let CameraSwitchPositionRequest {
            position,
            group,
            camera,
        } = request.into_inner();

        let position = request_impl::required_u16("position", position)?;
        let group = request_impl::required_u16("group", group)?;
        let camera = request_impl::required_u16("camera", camera)?;
        let span = tracing::Span::current();
        span.record("broadcast.position", position);
        span.record("broadcast.group", group);
        span.record("broadcast.camera", camera);
        let expected = CameraSelectionExpectation {
            car_index: None,
            group: u32::from(group),
            camera: u32::from(camera),
        };

        self.execute_observed(
            |observation| observation.camera_selection_snapshot(),
            BroadcastCommand::CameraSwitchPosition(position, group, camera),
            move |observation, previous, timeout| {
                observation.wait_for_camera_selection(previous, expected, timeout)
            },
            camera_switch_response,
        )
        .await
    }

    #[tracing::instrument(
        name = "grpc.camera_switch_number",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "CameraSwitchNumber",
            client.address = tracing::field::Empty,
            broadcast.car_number = tracing::field::Empty,
            broadcast.group = tracing::field::Empty,
            broadcast.camera = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn camera_switch_number(
        &self,
        request: Request<CameraSwitchNumberRequest>,
    ) -> Result<Response<CameraSwitchNumberResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let CameraSwitchNumberRequest {
            car_number,
            group,
            camera,
        } = request.into_inner();

        let car_number = request_impl::required_string("car_number", car_number)?;
        let group = request_impl::required_u16("group", group)?;
        let camera = request_impl::required_u16("camera", camera)?;
        let span = tracing::Span::current();
        span.record("broadcast.car_number", tracing::field::display(&car_number));
        span.record("broadcast.group", group);
        span.record("broadcast.camera", camera);
        let observation = self.observation()?;
        let previous = observation.camera_selection_snapshot()?;
        let car_index =
            observation.resolve_car_index_by_number(previous.session_version, &car_number)?;

        self.send_message(BroadcastCommand::CameraSwitchNumber(
            car_number, group, camera,
        ))?;

        let current = observation.wait_for_camera_selection(
            previous,
            CameraSelectionExpectation {
                car_index: Some(car_index),
                group: u32::from(group),
                camera: u32::from(camera),
            },
            self.observation_timeout,
        )?;

        let response = camera_switch_response(current)?;
        Ok(Response::new(CameraSwitchNumberResponse {
            car_index: response.car_index,
            group: response.group,
            camera: response.camera,
        }))
    }

    #[tracing::instrument(
        name = "grpc.camera_set_state",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "CameraSetState",
            client.address = tracing::field::Empty,
            broadcast.has_state = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn camera_set_state(
        &self,
        request: Request<CameraSetStateRequest>,
    ) -> Result<Response<CameraSetStateResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let CameraSetStateRequest { state } = request.into_inner();
        tracing::Span::current().record("broadcast.has_state", state.is_some());
        let _state = state.ok_or_else(|| Status::invalid_argument("Missing `state`"))?;
        Err(unsupported_state_resolution("camera_set_state"))
    }

    #[tracing::instrument(
        name = "grpc.replay_set_play_speed",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "ReplaySetPlaySpeed",
            client.address = tracing::field::Empty,
            replay.speed = tracing::field::Empty,
            replay.is_slow_motion = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn replay_set_play_speed(
        &self,
        request: Request<ReplaySetPlaySpeedRequest>,
    ) -> Result<Response<ReplaySetPlaySpeedResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let ReplaySetPlaySpeedRequest {
            speed,
            is_slow_motion,
        } = request.into_inner();

        let speed = request_impl::required_i16("speed", speed)?;
        let is_slow_motion = request_impl::required_bool("is_slow_motion", is_slow_motion)?;
        let span = tracing::Span::current();
        span.record("replay.speed", speed);
        span.record("replay.is_slow_motion", is_slow_motion);
        let expected = ReplaySpeedExpectation {
            speed: i32::from(speed),
            is_slow_motion,
        };

        self.execute_observed(
            |observation| observation.replay_speed_snapshot(),
            BroadcastCommand::ReplaySetPlaySpeed(speed, is_slow_motion),
            move |observation, previous, timeout| {
                observation.wait_for_replay_speed(previous, expected, timeout)
            },
            |current| Ok(replay_speed_response(current)),
        )
        .await
    }

    #[tracing::instrument(
        name = "grpc.replay_set_play_position",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "ReplaySetPlayPosition",
            client.address = tracing::field::Empty,
            replay.mode = tracing::field::Empty,
            replay.has_frame = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn replay_set_play_position(
        &self,
        request: Request<ReplaySetPlayPositionRequest>,
    ) -> Result<Response<ReplaySetPlayPositionResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let ReplaySetPlayPositionRequest { mode, frame } = request.into_inner();
        let span = tracing::Span::current();
        span.record(
            "replay.mode",
            tracing::field::display(format_args!("{mode:?}")),
        );
        span.record("replay.has_frame", frame.is_some());

        let _mode = request_impl::required_enum::<ReplayPositionMode>("mode", mode)?;
        let _frame = frame.ok_or_else(|| Status::invalid_argument("Missing `frame`"))?;

        Err(unsupported_state_resolution("replay_set_play_position"))
    }

    #[tracing::instrument(
        name = "grpc.replay_search",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "ReplaySearch",
            client.address = tracing::field::Empty,
            replay.mode = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn replay_search(
        &self,
        request: Request<ReplaySearchRequest>,
    ) -> Result<Response<ReplaySearchResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let ReplaySearchRequest { mode } = request.into_inner();
        tracing::Span::current().record(
            "replay.mode",
            tracing::field::display(format_args!("{mode:?}")),
        );

        let _mode = request_impl::required_enum::<ReplaySearchMode>("mode", mode)?;

        Err(unsupported_state_resolution("replay_search"))
    }

    #[tracing::instrument(
        name = "grpc.replay_set_state",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "ReplaySetState",
            client.address = tracing::field::Empty,
            replay.state = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn replay_set_state(
        &self,
        request: Request<ReplaySetStateRequest>,
    ) -> Result<Response<ReplaySetStateResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let ReplaySetStateRequest { state } = request.into_inner();
        tracing::Span::current().record(
            "replay.state",
            tracing::field::display(format_args!("{state:?}")),
        );

        let state = request_impl::required_enum::<ReplayStateMode>("state", state)?;

        self.execute_ack(
            BroadcastCommand::ReplaySetState(command_impl::replay_state_mode(state)),
            ReplaySetStateResponse {},
        )
        .await
    }

    #[tracing::instrument(
        name = "grpc.reload_textures",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "ReloadTextures",
            client.address = tracing::field::Empty,
            broadcast.car_idx = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn reload_textures(
        &self,
        request: Request<ReloadTexturesRequest>,
    ) -> Result<Response<ReloadTexturesResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let ReloadTexturesRequest { car_idx } = request.into_inner();
        tracing::Span::current().record(
            "broadcast.car_idx",
            tracing::field::display(format_args!("{car_idx:?}")),
        );

        self.execute_ack(
            match car_idx {
                Some(index) => request_impl::u32_to_u16("car_idx", index)
                    .map(BroadcastCommand::ReloadTextures)?,
                None => BroadcastCommand::ReloadAllTextures,
            },
            ReloadTexturesResponse {},
        )
        .await
    }

    #[tracing::instrument(
        name = "grpc.chat_command",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "ChatCommand",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn chat_command(
        &self,
        request: Request<ChatCommandRequest>,
    ) -> Result<Response<ChatCommandResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        self.execute_ack(
            command_impl::chat_command(request.into_inner())?,
            ChatCommandResponse {},
        )
        .await
    }

    #[tracing::instrument(
        name = "grpc.pit_command",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "PitCommand",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn pit_command(
        &self,
        request: Request<PitCommandRequest>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let _command = command_impl::pit_command(request.into_inner())?;
        Err(unsupported_state_resolution("pit_command"))
    }

    #[tracing::instrument(
        name = "grpc.pit_command_stream",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "PitCommandStream",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn pit_command_stream(
        &self,
        request: Request<tonic::Streaming<PitCommandRequest>>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let mut stream = request.into_inner();

        while let Some(request) = stream.message().await? {
            let _command = command_impl::pit_command(request)?;
        }

        Err(unsupported_state_resolution("pit_command_stream"))
    }

    #[tracing::instrument(
        name = "grpc.telemetry_command",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "TelemetryCommand",
            client.address = tracing::field::Empty,
            telemetry.mode = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn telemetry_command(
        &self,
        request: Request<TelemetryCommandRequest>,
    ) -> Result<Response<TelemetryCommandResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let TelemetryCommandRequest { mode } = request.into_inner();
        tracing::Span::current().record(
            "telemetry.mode",
            tracing::field::display(format_args!("{mode:?}")),
        );
        let _mode = request_impl::required_enum::<TelemetryCommandMode>("mode", mode)?;
        Err(unsupported_state_resolution("telemetry_command"))
    }

    #[tracing::instrument(
        name = "grpc.force_feedback_command",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "ForceFeedbackCommand",
            client.address = tracing::field::Empty,
            feedback.mode = tracing::field::Empty,
            feedback.value = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn force_feedback_command(
        &self,
        request: Request<ForceFeedbackCommandRequest>,
    ) -> Result<Response<ForceFeedbackCommandResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let ForceFeedbackCommandRequest { mode, value } = request.into_inner();
        let span = tracing::Span::current();
        span.record(
            "feedback.mode",
            tracing::field::display(format_args!("{mode:?}")),
        );
        span.record(
            "feedback.value",
            tracing::field::display(format_args!("{value:?}")),
        );

        let mode = request_impl::required_enum::<ForceFeedbackCommandMode>("mode", mode)?;
        let _value = request_impl::required_f32("value", value)?;

        match mode {
            ForceFeedbackCommandMode::MaxForce => {
                Err(unsupported_state_resolution("force_feedback_command"))
            }
            ForceFeedbackCommandMode::Unknown => {
                unreachable!("unknown force feedback command mode is rejected")
            }
        }
    }

    #[tracing::instrument(
        name = "grpc.replay_search_session_time",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "ReplaySearchSessionTime",
            client.address = tracing::field::Empty,
            replay.session_number = tracing::field::Empty,
            replay.has_session_time_ms = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn replay_search_session_time(
        &self,
        request: Request<ReplaySearchSessionTimeRequest>,
    ) -> Result<Response<ReplaySearchSessionTimeResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let ReplaySearchSessionTimeRequest {
            session_number,
            session_time_ms,
        } = request.into_inner();
        let span = tracing::Span::current();
        span.record(
            "replay.session_number",
            tracing::field::display(format_args!("{session_number:?}")),
        );
        span.record("replay.has_session_time_ms", session_time_ms.is_some());

        let session_number = request_impl::required_u16("session_number", session_number)?;
        let session_time_ms =
            session_time_ms.ok_or_else(|| Status::invalid_argument("Missing `session_time_ms`"))?;

        self.execute_ack(
            BroadcastCommand::ReplaySearchSessionTime(session_number, session_time_ms),
            ReplaySearchSessionTimeResponse {},
        )
        .await
    }

    #[tracing::instrument(
        name = "grpc.video_capture",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "VideoCapture",
            client.address = tracing::field::Empty,
            video.mode = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn video_capture(
        &self,
        request: Request<VideoCaptureRequest>,
    ) -> Result<Response<VideoCaptureResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let VideoCaptureRequest { mode } = request.into_inner();
        tracing::Span::current().record(
            "video.mode",
            tracing::field::display(format_args!("{mode:?}")),
        );

        let mode = request_impl::required_enum::<VideoCaptureMode>("mode", mode)?;

        self.execute_ack(
            BroadcastCommand::VideoCapture(command_impl::video_capture_mode(mode)),
            VideoCaptureResponse {},
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        sync::{Arc, Mutex as StdMutex},
    };

    use super::*;

    #[derive(Default)]
    struct FakeSender {
        sent: StdMutex<Vec<BroadcastCommand>>,
    }

    impl FakeSender {
        fn sent(&self) -> Vec<BroadcastCommand> {
            self.sent.lock().expect("sender mutex poisoned").clone()
        }
    }

    impl BroadcastCommandSender for FakeSender {
        fn send_message(&self, message: BroadcastCommand) -> Result<(), IRacingSDKError> {
            self.sent
                .lock()
                .expect("sender mutex poisoned")
                .push(message);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeObservation {
        available_camera_groups: StdMutex<VecDeque<Result<Vec<CameraGroup>, Status>>>,
        camera_snapshots:
            StdMutex<VecDeque<Result<ObservedValue<CameraSelectionTelemetry>, Status>>>,
        camera_waits: StdMutex<VecDeque<Result<ObservedValue<CameraSelectionTelemetry>, Status>>>,
        car_number_resolutions: StdMutex<HashMap<String, Result<u32, Status>>>,
        replay_snapshots: StdMutex<VecDeque<Result<ObservedValue<ReplaySpeedTelemetry>, Status>>>,
        replay_waits: StdMutex<VecDeque<Result<ObservedValue<ReplaySpeedTelemetry>, Status>>>,
    }

    impl FakeObservation {
        fn with_camera_snapshot(
            self,
            value: Result<ObservedValue<CameraSelectionTelemetry>, Status>,
        ) -> Self {
            self.camera_snapshots
                .lock()
                .expect("observation mutex poisoned")
                .push_back(value);
            self
        }

        fn with_camera_wait(
            self,
            value: Result<ObservedValue<CameraSelectionTelemetry>, Status>,
        ) -> Self {
            self.camera_waits
                .lock()
                .expect("observation mutex poisoned")
                .push_back(value);
            self
        }

        fn with_replay_snapshot(
            self,
            value: Result<ObservedValue<ReplaySpeedTelemetry>, Status>,
        ) -> Self {
            self.replay_snapshots
                .lock()
                .expect("observation mutex poisoned")
                .push_back(value);
            self
        }

        fn with_replay_wait(
            self,
            value: Result<ObservedValue<ReplaySpeedTelemetry>, Status>,
        ) -> Self {
            self.replay_waits
                .lock()
                .expect("observation mutex poisoned")
                .push_back(value);
            self
        }

        fn with_car_resolution(self, car_number: &str, car_index: Result<u32, Status>) -> Self {
            self.car_number_resolutions
                .lock()
                .expect("observation mutex poisoned")
                .insert(car_number.to_string(), car_index);
            self
        }
    }

    impl ObservationBackend for FakeObservation {
        fn available_camera_groups(
            &self,
            _session_version: u32,
        ) -> Result<Vec<CameraGroup>, Status> {
            self.available_camera_groups
                .lock()
                .expect("observation mutex poisoned")
                .pop_front()
                .unwrap_or_else(|| Ok(Vec::new()))
        }

        fn camera_selection_snapshot(
            &self,
        ) -> Result<ObservedValue<CameraSelectionTelemetry>, Status> {
            self.camera_snapshots
                .lock()
                .expect("observation mutex poisoned")
                .pop_front()
                .expect("camera snapshot should be configured")
        }

        fn wait_for_camera_selection(
            &self,
            _previous: ObservedValue<CameraSelectionTelemetry>,
            _expected: CameraSelectionExpectation,
            _timeout: Duration,
        ) -> Result<ObservedValue<CameraSelectionTelemetry>, Status> {
            self.camera_waits
                .lock()
                .expect("observation mutex poisoned")
                .pop_front()
                .expect("camera wait should be configured")
        }

        fn resolve_car_index_by_number(
            &self,
            _session_version: u32,
            car_number: &str,
        ) -> Result<u32, Status> {
            self.car_number_resolutions
                .lock()
                .expect("observation mutex poisoned")
                .get(car_number)
                .cloned()
                .unwrap_or_else(|| {
                    Err(Status::failed_precondition(format!(
                        "no fake car resolution configured for `{car_number}`"
                    )))
                })
        }

        fn replay_speed_snapshot(&self) -> Result<ObservedValue<ReplaySpeedTelemetry>, Status> {
            self.replay_snapshots
                .lock()
                .expect("observation mutex poisoned")
                .pop_front()
                .expect("replay snapshot should be configured")
        }

        fn wait_for_replay_speed(
            &self,
            _previous: ObservedValue<ReplaySpeedTelemetry>,
            _expected: ReplaySpeedExpectation,
            _timeout: Duration,
        ) -> Result<ObservedValue<ReplaySpeedTelemetry>, Status> {
            self.replay_waits
                .lock()
                .expect("observation mutex poisoned")
                .pop_front()
                .expect("replay wait should be configured")
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

    fn service_with(
        sender: Arc<FakeSender>,
        observation: Option<Arc<FakeObservation>>,
    ) -> BroadcastService {
        BroadcastService {
            sender,
            observation: observation.map(|observation| observation as Arc<dyn ObservationBackend>),
            observation_timeout: Duration::from_millis(25),
        }
    }

    #[tokio::test]
    async fn camera_switch_position_observes_and_returns_current_state() {
        let sender = Arc::new(FakeSender::default());
        let observation = Arc::new(
            FakeObservation::default()
                .with_camera_snapshot(Ok(observed_camera(7, 1, 2, 3)))
                .with_camera_wait(Ok(observed_camera(7, 42, 4, 5))),
        );
        let service = service_with(Arc::clone(&sender), Some(observation));

        let response = service
            .camera_switch_position(Request::new(CameraSwitchPositionRequest {
                position: Some(42),
                group: Some(4),
                camera: Some(5),
            }))
            .await
            .expect("camera switch should succeed")
            .into_inner();

        assert_eq!(response.car_index, 42);
        assert_eq!(response.group, 4);
        assert_eq!(response.camera, 5);
        assert_eq!(
            sender.sent(),
            vec![BroadcastCommand::CameraSwitchPosition(42, 4, 5)]
        );
    }

    #[tokio::test]
    async fn camera_switch_number_resolves_car_index_and_returns_observed_state() {
        let sender = Arc::new(FakeSender::default());
        let observation = Arc::new(
            FakeObservation::default()
                .with_camera_snapshot(Ok(observed_camera(11, 1, 2, 3)))
                .with_camera_wait(Ok(observed_camera(11, 12, 6, 7)))
                .with_car_resolution("012", Ok(12)),
        );
        let service = service_with(Arc::clone(&sender), Some(observation));

        let response = service
            .camera_switch_number(Request::new(CameraSwitchNumberRequest {
                car_number: Some("012".to_string()),
                group: Some(6),
                camera: Some(7),
            }))
            .await
            .expect("camera switch should succeed")
            .into_inner();

        assert_eq!(response.car_index, 12);
        assert_eq!(response.group, 6);
        assert_eq!(response.camera, 7);
        assert_eq!(
            sender.sent(),
            vec![BroadcastCommand::CameraSwitchNumber(
                "012".to_string(),
                6,
                7
            )]
        );
    }

    #[tokio::test]
    async fn replay_set_play_speed_observes_and_returns_current_state() {
        let sender = Arc::new(FakeSender::default());
        let observation = Arc::new(
            FakeObservation::default()
                .with_replay_snapshot(Ok(observed_replay(5, 0, false)))
                .with_replay_wait(Ok(observed_replay(5, 2, true))),
        );
        let service = service_with(Arc::clone(&sender), Some(observation));

        let response = service
            .replay_set_play_speed(Request::new(ReplaySetPlaySpeedRequest {
                speed: Some(2),
                is_slow_motion: Some(true),
            }))
            .await
            .expect("replay speed should succeed")
            .into_inner();

        assert_eq!(response.speed, 2);
        assert!(response.is_slow_motion);
        assert_eq!(
            sender.sent(),
            vec![BroadcastCommand::ReplaySetPlaySpeed(2, true)]
        );
    }

    #[tokio::test]
    async fn observed_rpc_fails_fast_without_observer() {
        let sender = Arc::new(FakeSender::default());
        let service = service_with(sender, None);

        let error = service
            .camera_switch_position(Request::new(CameraSwitchPositionRequest {
                position: Some(1),
                group: Some(2),
                camera: Some(3),
            }))
            .await
            .expect_err("camera switch should fail without observation");

        assert_eq!(error.code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn observed_rpc_propagates_timeout() {
        let sender = Arc::new(FakeSender::default());
        let observation = Arc::new(
            FakeObservation::default()
                .with_camera_snapshot(Ok(observed_camera(7, 1, 2, 3)))
                .with_camera_wait(Err(Status::deadline_exceeded("timed out"))),
        );
        let service = service_with(sender, Some(observation));

        let error = service
            .camera_switch_position(Request::new(CameraSwitchPositionRequest {
                position: Some(4),
                group: Some(5),
                camera: Some(6),
            }))
            .await
            .expect_err("camera switch should time out");

        assert_eq!(error.code(), tonic::Code::DeadlineExceeded);
    }

    #[tokio::test]
    async fn chat_command_uses_ack_path_without_observation() {
        let sender = Arc::new(FakeSender::default());
        let service = service_with(Arc::clone(&sender), None);

        service
            .chat_command(Request::new(ChatCommandRequest {
                mode: Some(ChatCommandMode::Macro as i32),
                r#macro: Some(3),
            }))
            .await
            .expect("chat command should succeed");

        assert_eq!(sender.sent(), vec![BroadcastCommand::ChatCommandMacro(3)]);
    }
}
