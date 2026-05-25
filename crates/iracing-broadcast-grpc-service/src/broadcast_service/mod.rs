mod builder;
mod command;
mod error;
mod request;
mod response;

use std::{future::Future, sync::Arc, time::Duration};

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::broadcast::broadcast_server::Broadcast;
use crate::broadcast::*;
use crate::broadcast_app::{BroadcastError, BroadcastUseCases};

pub use builder::BroadcastServiceBuilder;
use command as command_impl;
use error::broadcast_error_to_status;
use request as request_impl;
use response as response_impl;

const DEFAULT_OBSERVATION_TIMEOUT: Duration = Duration::from_secs(2);
const SUBSCRIPTION_BUFFER_CAPACITY: usize = 16;
const SUBSCRIPTION_POLL_INTERVAL: Duration = Duration::from_millis(50);

type CurrentValueStream<T> = ReceiverStream<Result<T, Status>>;

fn current_value_stream<S, R, Fut, Snapshot, Map>(
    use_cases: Arc<BroadcastUseCases>,
    mut snapshot: Snapshot,
    map: Map,
) -> CurrentValueStream<R>
where
    Fut: Future<Output = Result<S, BroadcastError>> + Send + 'static,
    Snapshot: FnMut(Arc<BroadcastUseCases>) -> Fut + Send + 'static,
    Map: Fn(S) -> R + Send + 'static,
    S: Send + 'static,
    R: Clone + PartialEq + Send + 'static,
{
    let (tx, rx) = mpsc::channel(SUBSCRIPTION_BUFFER_CAPACITY);

    tokio::spawn(async move {
        let mut previous = None;

        loop {
            if tx.is_closed() {
                break;
            }

            let response = match snapshot(Arc::clone(&use_cases)).await {
                Ok(snapshot) => map(snapshot),
                Err(error) => {
                    let _ = tx.send(Err(Status::from(error))).await;
                    break;
                }
            };

            if previous.as_ref() == Some(&response) {
                tokio::time::sleep(SUBSCRIPTION_POLL_INTERVAL);
                continue;
            }

            previous = Some(response.clone());
            if tx.send(Ok(response)).await.is_err() {
                break;
            }
        }
    });

    ReceiverStream::new(rx)
}

/// Tonic adapter that serves the iRacing broadcast gRPC API on Windows.
///
/// `BroadcastService` is intentionally thin: request parsing and protobuf
/// response mapping stay at this boundary, while command orchestration is
/// delegated to internal use cases. Construct it with [`BroadcastService::new`]
/// or [`BroadcastService::builder`].
pub struct BroadcastService {
    use_cases: Arc<BroadcastUseCases>,
}

impl BroadcastService {
    /// Create a builder for configuring the live broadcast service.
    pub fn builder() -> crate::BroadcastServiceBuilder {
        crate::BroadcastServiceBuilder::default()
    }

    /// Create a live broadcast service with default settings.
    ///
    /// This opens the iRacing Win32 broadcast channel and live telemetry
    /// observation path. Use [`BroadcastServiceBuilder::without_observation`] if
    /// the service should expose ack-only commands without telemetry-backed
    /// state resolution.
    pub fn new() -> Result<Self, Status> {
        Self::builder().build().map_err(broadcast_error_to_status)
    }

    pub(crate) fn from_use_cases(use_cases: Arc<BroadcastUseCases>) -> Self {
        Self { use_cases }
    }
}

#[tonic::async_trait]
impl Broadcast for BroadcastService {
    type SubscribeCurrentCameraPositionStream = CurrentValueStream<CurrentCameraPositionResponse>;
    type SubscribeCurrentCameraStateStream = CurrentValueStream<CurrentCameraStateResponse>;
    type SubscribeCurrentReplayPlaySpeedStream = CurrentValueStream<CurrentReplayPlaySpeedResponse>;
    type SubscribeCurrentReplayPositionStream = CurrentValueStream<CurrentReplayPositionResponse>;
    type SubscribeCurrentPitServiceStream = CurrentValueStream<CurrentPitServiceResponse>;
    type SubscribeCurrentTelemetryStateStream = CurrentValueStream<CurrentTelemetryStateResponse>;
    type SubscribeCurrentForceFeedbackStream = CurrentValueStream<CurrentForceFeedbackResponse>;
    type SubscribeCurrentVideoCaptureStream = CurrentValueStream<CurrentVideoCaptureResponse>;

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

        let cameras = self
            .use_cases
            .get_available_cameras()
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::available_cameras(cameras)))
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

        let position = request_impl::optional_u16("position", position)?;
        let group = request_impl::optional_u16("group", group)?;
        let camera = request_impl::optional_u16("camera", camera)?;
        let span = tracing::Span::current();
        span.record(
            "broadcast.position",
            tracing::field::display(format_args!("{position:?}")),
        );
        span.record(
            "broadcast.group",
            tracing::field::display(format_args!("{group:?}")),
        );
        span.record(
            "broadcast.camera",
            tracing::field::display(format_args!("{camera:?}")),
        );

        let snapshot = self
            .use_cases
            .camera_switch_position(position, group, camera)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::camera_switch_position(
            snapshot,
        )))
    }

    #[tracing::instrument(
        name = "grpc.current_camera_position",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "CurrentCameraPosition",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn current_camera_position(
        &self,
        request: Request<()>,
    ) -> Result<Response<CurrentCameraPositionResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        let snapshot = self
            .use_cases
            .current_camera_position()
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::current_camera_position(
            snapshot,
        )))
    }

    #[tracing::instrument(
        name = "grpc.subscribe_current_camera_position",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "SubscribeCurrentCameraPosition",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn subscribe_current_camera_position(
        &self,
        request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentCameraPositionStream>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        Ok(Response::new(current_value_stream(
            Arc::clone(&self.use_cases),
            |use_cases| async move { use_cases.current_camera_position().await },
            response_impl::current_camera_position,
        )))
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

        let car_number = request_impl::optional_string("car_number", car_number)?;
        let group = request_impl::optional_u16("group", group)?;
        let camera = request_impl::optional_u16("camera", camera)?;
        let span = tracing::Span::current();
        span.record(
            "broadcast.car_number",
            tracing::field::display(format_args!("{car_number:?}")),
        );
        span.record(
            "broadcast.group",
            tracing::field::display(format_args!("{group:?}")),
        );
        span.record(
            "broadcast.camera",
            tracing::field::display(format_args!("{camera:?}")),
        );

        let snapshot = self
            .use_cases
            .camera_switch_number(car_number, group, camera)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::camera_switch_number(snapshot)))
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
        let state = request_impl::optional_u32(state).map(command_impl::camera_state);

        let snapshot = self
            .use_cases
            .camera_set_state(state)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::camera_set_state(snapshot)))
    }

    #[tracing::instrument(
        name = "grpc.current_camera_state",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "CurrentCameraState",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn current_camera_state(
        &self,
        request: Request<()>,
    ) -> Result<Response<CurrentCameraStateResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        let snapshot = self
            .use_cases
            .current_camera_state()
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::current_camera_state(snapshot)))
    }

    #[tracing::instrument(
        name = "grpc.subscribe_current_camera_state",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "SubscribeCurrentCameraState",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn subscribe_current_camera_state(
        &self,
        request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentCameraStateStream>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        Ok(Response::new(current_value_stream(
            Arc::clone(&self.use_cases),
            |use_cases| async move { use_cases.current_camera_state().await },
            response_impl::current_camera_state,
        )))
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

        let speed = request_impl::optional_i16("speed", speed)?;
        let span = tracing::Span::current();
        span.record(
            "replay.speed",
            tracing::field::display(format_args!("{speed:?}")),
        );
        span.record(
            "replay.is_slow_motion",
            tracing::field::display(format_args!("{is_slow_motion:?}")),
        );

        let snapshot = self
            .use_cases
            .replay_set_play_speed(speed, is_slow_motion)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::replay_set_play_speed(
            snapshot,
        )))
    }

    #[tracing::instrument(
        name = "grpc.current_replay_play_speed",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "CurrentReplayPlaySpeed",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn current_replay_play_speed(
        &self,
        request: Request<()>,
    ) -> Result<Response<CurrentReplayPlaySpeedResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        let snapshot = self
            .use_cases
            .current_replay_play_speed()
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::current_replay_play_speed(
            snapshot,
        )))
    }

    #[tracing::instrument(
        name = "grpc.subscribe_current_replay_play_speed",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "SubscribeCurrentReplayPlaySpeed",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn subscribe_current_replay_play_speed(
        &self,
        request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentReplayPlaySpeedStream>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        Ok(Response::new(current_value_stream(
            Arc::clone(&self.use_cases),
            |use_cases| async move { use_cases.current_replay_play_speed().await },
            response_impl::current_replay_play_speed,
        )))
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

        let mode = request_impl::required_enum::<ReplayPositionMode>("mode", mode)?;
        let frame = request_impl::required_u32("frame", frame)?;

        let snapshot = self
            .use_cases
            .replay_set_play_position(command_impl::replay_position_mode(mode), frame)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::replay_set_play_position(
            snapshot,
        )))
    }

    #[tracing::instrument(
        name = "grpc.current_replay_position",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "CurrentReplayPosition",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn current_replay_position(
        &self,
        request: Request<()>,
    ) -> Result<Response<CurrentReplayPositionResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        let snapshot = self
            .use_cases
            .current_replay_position()
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::current_replay_position(
            snapshot,
        )))
    }

    #[tracing::instrument(
        name = "grpc.subscribe_current_replay_position",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "SubscribeCurrentReplayPosition",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn subscribe_current_replay_position(
        &self,
        request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentReplayPositionStream>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        Ok(Response::new(current_value_stream(
            Arc::clone(&self.use_cases),
            |use_cases| async move { use_cases.current_replay_position().await },
            response_impl::current_replay_position,
        )))
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

        let mode = request_impl::required_enum::<ReplaySearchMode>("mode", mode)?;

        let snapshot = self
            .use_cases
            .replay_search(command_impl::replay_search_mode(mode))
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::replay_search(snapshot)))
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

        self.use_cases
            .replay_set_state(command_impl::replay_state_mode(state))
            .await
            .map_err(Status::from)?;

        Ok(Response::new(ReplaySetStateResponse {}))
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

        let car_idx = car_idx
            .map(|index| request_impl::u32_to_u16("car_idx", index))
            .transpose()?;

        self.use_cases
            .reload_textures(car_idx)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(ReloadTexturesResponse {}))
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
        self.use_cases
            .chat_command(command_impl::chat_command(request.into_inner())?)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(ChatCommandResponse {}))
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
        let snapshot = self
            .use_cases
            .pit_command(command_impl::pit_command(request.into_inner())?)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::pit_command(snapshot)))
    }

    #[tracing::instrument(
        name = "grpc.current_pit_service",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "CurrentPitService",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn current_pit_service(
        &self,
        request: Request<()>,
    ) -> Result<Response<CurrentPitServiceResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        let snapshot = self
            .use_cases
            .current_pit_service()
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::current_pit_service(snapshot)))
    }

    #[tracing::instrument(
        name = "grpc.subscribe_current_pit_service",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "SubscribeCurrentPitService",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn subscribe_current_pit_service(
        &self,
        request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentPitServiceStream>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        Ok(Response::new(current_value_stream(
            Arc::clone(&self.use_cases),
            |use_cases| async move { use_cases.current_pit_service().await },
            response_impl::current_pit_service,
        )))
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
        const MAX_PIT_COMMANDS: usize = 1000;

        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );
        let mut stream = request.into_inner();

        let mut commands = Vec::new();
        while let Some(request) = stream.message().await? {
            commands.push(command_impl::pit_command(request)?);
            if commands.len() > MAX_PIT_COMMANDS {
                return Err(Status::resource_exhausted(format!(
                    "pit_command_stream exceeds maximum of {} commands",
                    MAX_PIT_COMMANDS
                )));
            }
        }

        if commands.is_empty() {
            return Err(Status::invalid_argument(
                "`pit_command_stream` requires at least one command",
            ));
        }

        let snapshot = self
            .use_cases
            .pit_command_stream(commands)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::pit_command(snapshot)))
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
        let mode = request_impl::required_enum::<TelemetryCommandMode>("mode", mode)?;

        let snapshot = self
            .use_cases
            .telemetry_command(command_impl::telemetry_command_mode(mode))
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::telemetry_command(snapshot)))
    }

    #[tracing::instrument(
        name = "grpc.current_telemetry_state",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "CurrentTelemetryState",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn current_telemetry_state(
        &self,
        request: Request<()>,
    ) -> Result<Response<CurrentTelemetryStateResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        let snapshot = self
            .use_cases
            .current_telemetry_state()
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::current_telemetry_state(
            snapshot,
        )))
    }

    #[tracing::instrument(
        name = "grpc.subscribe_current_telemetry_state",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "SubscribeCurrentTelemetryState",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn subscribe_current_telemetry_state(
        &self,
        request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentTelemetryStateStream>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        Ok(Response::new(current_value_stream(
            Arc::clone(&self.use_cases),
            |use_cases| async move { use_cases.current_telemetry_state().await },
            response_impl::current_telemetry_state,
        )))
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
        let value = request_impl::required_f32("value", value)?;

        match mode {
            ForceFeedbackCommandMode::MaxForce => {
                let snapshot = self
                    .use_cases
                    .force_feedback_command(value)
                    .await
                    .map_err(Status::from)?;

                Ok(Response::new(response_impl::force_feedback(snapshot)))
            }
            ForceFeedbackCommandMode::Unknown => {
                unreachable!("unknown force feedback command mode is rejected")
            }
        }
    }

    #[tracing::instrument(
        name = "grpc.current_force_feedback",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "CurrentForceFeedback",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn current_force_feedback(
        &self,
        request: Request<()>,
    ) -> Result<Response<CurrentForceFeedbackResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        let snapshot = self
            .use_cases
            .current_force_feedback()
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::current_force_feedback(
            snapshot,
        )))
    }

    #[tracing::instrument(
        name = "grpc.subscribe_current_force_feedback",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "SubscribeCurrentForceFeedback",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn subscribe_current_force_feedback(
        &self,
        request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentForceFeedbackStream>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        Ok(Response::new(current_value_stream(
            Arc::clone(&self.use_cases),
            |use_cases| async move { use_cases.current_force_feedback().await },
            response_impl::current_force_feedback,
        )))
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

        self.use_cases
            .replay_search_session_time(session_number, session_time_ms)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(ReplaySearchSessionTimeResponse {}))
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

        self.use_cases
            .video_capture(command_impl::video_capture_mode(mode))
            .await
            .map_err(Status::from)?;

        Ok(Response::new(VideoCaptureResponse {}))
    }

    #[tracing::instrument(
        name = "grpc.current_video_capture",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "CurrentVideoCapture",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn current_video_capture(
        &self,
        request: Request<()>,
    ) -> Result<Response<CurrentVideoCaptureResponse>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        let snapshot = self
            .use_cases
            .current_video_capture()
            .await
            .map_err(Status::from)?;

        Ok(Response::new(response_impl::current_video_capture(
            snapshot,
        )))
    }

    #[tracing::instrument(
        name = "grpc.subscribe_current_video_capture",
        skip_all,
        fields(
            rpc.system = "grpc",
            rpc.service = "iracing.broadcast.Broadcast",
            rpc.method = "SubscribeCurrentVideoCapture",
            client.address = tracing::field::Empty
        ),
        err(level = tracing::Level::WARN)
    )]
    async fn subscribe_current_video_capture(
        &self,
        request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentVideoCaptureStream>, Status> {
        tracing::Span::current().record(
            "client.address",
            tracing::field::display(format_args!("{:?}", request.remote_addr())),
        );

        Ok(Response::new(current_value_stream(
            Arc::clone(&self.use_cases),
            |use_cases| async move { use_cases.current_video_capture().await },
            response_impl::current_video_capture,
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        sync::{Arc, Mutex as StdMutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use iracing_sdk::{BroadcastCommand, IRacingSDKError};
    use tokio_stream::StreamExt;

    use super::*;
    use crate::broadcast_app::{
        AvailableCamera, AvailableCameraGroup, BroadcastCommandPort, BroadcastError,
        CameraSelectionExpectation, CameraSelectionSnapshot, CameraStateExpectation,
        CameraStatePort, CameraStateSnapshot, DisabledObservationPort, ReplayPlayStateSnapshot,
        ReplayPositionExpectation, ReplayPositionSnapshot, ReplaySpeedExpectation,
        ReplaySpeedSnapshot, ReplayStatePort,
    };

    #[derive(Default)]
    struct FakeCommands {
        sent: StdMutex<Vec<BroadcastCommand>>,
        error: StdMutex<Option<BroadcastError>>,
    }

    impl FakeCommands {
        fn with_error(error: BroadcastError) -> Self {
            Self {
                sent: StdMutex::new(Vec::new()),
                error: StdMutex::new(Some(error)),
            }
        }

        fn sent(&self) -> Vec<BroadcastCommand> {
            self.sent.lock().expect("sender mutex poisoned").clone()
        }
    }

    #[async_trait]
    impl BroadcastCommandPort for FakeCommands {
        async fn send(&self, command: BroadcastCommand) -> Result<(), BroadcastError> {
            self.sent
                .lock()
                .expect("sender mutex poisoned")
                .push(command);

            if let Some(error) = self.error.lock().expect("sender mutex poisoned").take() {
                return Err(error);
            }

            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeCamera {
        snapshots: StdMutex<VecDeque<Result<CameraSelectionSnapshot, BroadcastError>>>,
        waits: StdMutex<VecDeque<Result<CameraSelectionSnapshot, BroadcastError>>>,
        groups: StdMutex<VecDeque<Result<Vec<AvailableCameraGroup>, BroadcastError>>>,
        resolutions: StdMutex<HashMap<String, u32>>,
    }

    impl FakeCamera {
        fn with_snapshot(self, value: Result<CameraSelectionSnapshot, BroadcastError>) -> Self {
            self.snapshots
                .lock()
                .expect("camera mutex poisoned")
                .push_back(value);
            self
        }

        fn with_wait(self, value: Result<CameraSelectionSnapshot, BroadcastError>) -> Self {
            self.waits
                .lock()
                .expect("camera mutex poisoned")
                .push_back(value);
            self
        }

        fn with_groups(self, value: Result<Vec<AvailableCameraGroup>, BroadcastError>) -> Self {
            self.groups
                .lock()
                .expect("camera mutex poisoned")
                .push_back(value);
            self
        }

        fn with_resolution(self, car_number: &str, car_index: u32) -> Self {
            self.resolutions
                .lock()
                .expect("camera mutex poisoned")
                .insert(car_number.to_string(), car_index);
            self
        }
    }

    #[async_trait]
    impl CameraStatePort for FakeCamera {
        async fn selection_snapshot(&self) -> Result<CameraSelectionSnapshot, BroadcastError> {
            self.snapshots
                .lock()
                .expect("camera mutex poisoned")
                .pop_front()
                .expect("camera snapshot should be configured")
        }

        async fn wait_for_selection(
            &self,
            _previous: CameraSelectionSnapshot,
            _expected: CameraSelectionExpectation,
            _timeout: Duration,
        ) -> Result<CameraSelectionSnapshot, BroadcastError> {
            self.waits
                .lock()
                .expect("camera mutex poisoned")
                .pop_front()
                .expect("camera wait should be configured")
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
            self.groups
                .lock()
                .expect("camera mutex poisoned")
                .pop_front()
                .unwrap_or_else(|| Ok(Vec::new()))
        }

        async fn resolve_car_index_by_number(
            &self,
            _session_version: u32,
            car_number: &str,
        ) -> Result<u32, BroadcastError> {
            self.resolutions
                .lock()
                .expect("camera mutex poisoned")
                .get(car_number)
                .copied()
                .ok_or_else(|| {
                    BroadcastError::FailedPrecondition(format!(
                        "no fake car resolution configured for `{car_number}`"
                    ))
                })
        }

        async fn resolve_car_number_by_index(
            &self,
            _session_version: u32,
            car_index: u32,
        ) -> Result<String, BroadcastError> {
            self.resolutions
                .lock()
                .expect("camera mutex poisoned")
                .iter()
                .find_map(|(number, &index)| (index == car_index).then(|| number.clone()))
                .ok_or_else(|| {
                    BroadcastError::FailedPrecondition(format!(
                        "no fake car resolution configured for index `{car_index}`"
                    ))
                })
        }
    }

    #[derive(Default)]
    struct FakeReplay {
        snapshots: StdMutex<VecDeque<Result<ReplaySpeedSnapshot, BroadcastError>>>,
        waits: StdMutex<VecDeque<Result<ReplaySpeedSnapshot, BroadcastError>>>,
    }

    impl FakeReplay {
        fn with_snapshot(self, value: Result<ReplaySpeedSnapshot, BroadcastError>) -> Self {
            self.snapshots
                .lock()
                .expect("replay mutex poisoned")
                .push_back(value);
            self
        }

        fn with_wait(self, value: Result<ReplaySpeedSnapshot, BroadcastError>) -> Self {
            self.waits
                .lock()
                .expect("replay mutex poisoned")
                .push_back(value);
            self
        }
    }

    #[async_trait]
    impl ReplayStatePort for FakeReplay {
        async fn speed_snapshot(&self) -> Result<ReplaySpeedSnapshot, BroadcastError> {
            self.snapshots
                .lock()
                .expect("replay mutex poisoned")
                .pop_front()
                .expect("replay snapshot should be configured")
        }

        async fn play_state_snapshot(&self) -> Result<ReplayPlayStateSnapshot, BroadcastError> {
            let snapshot = self
                .snapshots
                .lock()
                .expect("replay mutex poisoned")
                .pop_front()
                .expect("replay snapshot should be configured")?;

            Ok(ReplayPlayStateSnapshot {
                speed: snapshot.speed,
                is_slow_motion: snapshot.is_slow_motion,
                is_playing: true,
            })
        }

        async fn wait_for_speed(
            &self,
            _previous: ReplaySpeedSnapshot,
            _expected: ReplaySpeedExpectation,
            _timeout: Duration,
        ) -> Result<ReplaySpeedSnapshot, BroadcastError> {
            self.waits
                .lock()
                .expect("replay mutex poisoned")
                .pop_front()
                .expect("replay wait should be configured")
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

    fn camera_snapshot(
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

    fn replay_snapshot(speed: i32, is_slow_motion: bool) -> ReplaySpeedSnapshot {
        ReplaySpeedSnapshot {
            speed,
            is_slow_motion,
        }
    }

    fn service_with(
        commands: Arc<FakeCommands>,
        camera: Option<Arc<FakeCamera>>,
        replay: Option<Arc<FakeReplay>>,
    ) -> BroadcastService {
        let commands: Arc<dyn BroadcastCommandPort> = commands;

        match (camera, replay) {
            (Some(camera), Some(replay)) => {
                let camera: Arc<dyn CameraStatePort> = camera;
                let replay: Arc<dyn ReplayStatePort> = replay;
                let disabled = Arc::new(DisabledObservationPort);
                BroadcastService::from_use_cases(Arc::new(BroadcastUseCases::new(
                    commands,
                    camera,
                    replay,
                    disabled.clone(),
                    disabled.clone(),
                    disabled.clone(),
                    disabled,
                    Duration::from_millis(25),
                )))
            }
            (None, None) => {
                let disabled = Arc::new(DisabledObservationPort);
                BroadcastService::from_use_cases(Arc::new(BroadcastUseCases::new(
                    commands,
                    disabled.clone(),
                    disabled.clone(),
                    disabled.clone(),
                    disabled.clone(),
                    disabled.clone(),
                    disabled,
                    Duration::from_millis(25),
                )))
            }
            _ => panic!("camera and replay fakes must be provided together"),
        }
    }

    fn assert_invalid_argument(error: Status, field: &str) {
        assert_eq!(error.code(), tonic::Code::InvalidArgument);
        assert!(
            error.message().contains(field),
            "error message should mention `{field}`: {}",
            error.message()
        );
    }

    #[tokio::test]
    async fn get_available_cameras_maps_domain_response() {
        let commands = Arc::new(FakeCommands::default());
        let camera = Arc::new(
            FakeCamera::default()
                .with_snapshot(Ok(camera_snapshot(7, 42, 4, 5)))
                .with_groups(Ok(vec![AvailableCameraGroup {
                    number: 4,
                    name: "TV".to_string(),
                    cameras: vec![AvailableCamera {
                        number: 5,
                        name: Some("Nose".to_string()),
                    }],
                }])),
        );
        let replay = Arc::new(FakeReplay::default());
        let service = service_with(commands, Some(camera), Some(replay));

        let response = service
            .get_available_cameras(Request::new(()))
            .await
            .expect("available cameras should succeed")
            .into_inner();

        assert_eq!(response.car_index, 42);
        assert_eq!(response.group, 4);
        assert_eq!(response.camera, 5);
        assert_eq!(response.camera_groups.len(), 1);
        assert_eq!(response.camera_groups[0].number, 4);
        assert_eq!(response.camera_groups[0].cameras[0].number, Some(5));
    }

    #[tokio::test]
    async fn subscribe_current_camera_position_emits_initial_value_then_changes() {
        let commands = Arc::new(FakeCommands::default());
        let camera = Arc::new(
            FakeCamera::default()
                .with_snapshot(Ok(camera_snapshot(7, 42, 4, 5)))
                .with_snapshot(Ok(camera_snapshot(7, 42, 4, 5)))
                .with_snapshot(Ok(camera_snapshot(7, 43, 4, 5)))
                .with_snapshot(Err(BroadcastError::ObservationSourceEnded)),
        );
        let replay = Arc::new(FakeReplay::default());
        let service = service_with(commands, Some(camera), Some(replay));

        let mut stream = service
            .subscribe_current_camera_position(Request::new(()))
            .await
            .expect("subscription should start")
            .into_inner();

        let first = stream
            .next()
            .await
            .expect("first stream item should exist")
            .expect("first stream item should be ok");
        assert_eq!(first.car_index, 42);
        assert_eq!(first.group, 4);
        assert_eq!(first.camera, 5);

        let second = stream
            .next()
            .await
            .expect("second stream item should exist")
            .expect("second stream item should be ok");
        assert_eq!(second.car_index, 43);
        assert_eq!(second.group, 4);
        assert_eq!(second.camera, 5);

        let error = stream
            .next()
            .await
            .expect("terminal stream item should exist")
            .expect_err("source end should terminate stream with a status");
        assert_eq!(error.code(), tonic::Code::Unavailable);
    }

    #[tokio::test]
    async fn camera_switch_position_observes_and_returns_current_state() {
        let commands = Arc::new(FakeCommands::default());
        let camera = Arc::new(
            FakeCamera::default()
                .with_snapshot(Ok(camera_snapshot(7, 1, 2, 3)))
                .with_wait(Ok(camera_snapshot(7, 42, 4, 5))),
        );
        let replay = Arc::new(FakeReplay::default());
        let service = service_with(Arc::clone(&commands), Some(camera), Some(replay));

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
            commands.sent(),
            vec![BroadcastCommand::CameraSwitchPosition(42, 4, 5)]
        );
    }

    #[tokio::test]
    async fn camera_switch_position_uses_current_telemetry_for_missing_fields() {
        let commands = Arc::new(FakeCommands::default());
        let camera = Arc::new(
            FakeCamera::default()
                .with_snapshot(Ok(camera_snapshot(7, 42, 4, 5)))
                .with_wait(Ok(camera_snapshot(7, 42, 4, 8))),
        );
        let replay = Arc::new(FakeReplay::default());
        let service = service_with(Arc::clone(&commands), Some(camera), Some(replay));

        let response = service
            .camera_switch_position(Request::new(CameraSwitchPositionRequest {
                position: None,
                group: None,
                camera: Some(8),
            }))
            .await
            .expect("partial camera switch should succeed")
            .into_inner();

        assert_eq!(response.car_index, 42);
        assert_eq!(response.group, 4);
        assert_eq!(response.camera, 8);
        assert_eq!(
            commands.sent(),
            vec![BroadcastCommand::CameraSwitchPosition(42, 4, 8)]
        );
    }

    #[tokio::test]
    async fn camera_switch_position_rejects_invalid_proto_fields_before_dependencies() {
        let cases = [(
            "position",
            CameraSwitchPositionRequest {
                position: Some(u32::from(u16::MAX) + 1),
                group: Some(1),
                camera: Some(1),
            },
        )];

        for (field, request) in cases {
            let commands = Arc::new(FakeCommands::default());
            let service = service_with(Arc::clone(&commands), None, None);

            let error = service
                .camera_switch_position(Request::new(request))
                .await
                .expect_err("invalid request should fail");

            assert_invalid_argument(error, field);
            assert!(
                commands.sent().is_empty(),
                "invalid `{field}` should return before dependency calls"
            );
        }
    }

    #[tokio::test]
    async fn camera_switch_number_resolves_car_index_and_returns_observed_state() {
        let commands = Arc::new(FakeCommands::default());
        let camera = Arc::new(
            FakeCamera::default()
                .with_snapshot(Ok(camera_snapshot(11, 1, 2, 3)))
                .with_wait(Ok(camera_snapshot(11, 12, 6, 7)))
                .with_resolution("012", 12),
        );
        let replay = Arc::new(FakeReplay::default());
        let service = service_with(Arc::clone(&commands), Some(camera), Some(replay));

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
            commands.sent(),
            vec![BroadcastCommand::CameraSwitchNumber(
                "012".to_string(),
                6,
                7
            )]
        );
    }

    #[tokio::test]
    async fn camera_switch_number_uses_current_telemetry_for_missing_fields() {
        let commands = Arc::new(FakeCommands::default());
        let camera = Arc::new(
            FakeCamera::default()
                .with_snapshot(Ok(camera_snapshot(11, 12, 6, 7)))
                .with_wait(Ok(camera_snapshot(11, 12, 6, 8)))
                .with_resolution("012", 12),
        );
        let replay = Arc::new(FakeReplay::default());
        let service = service_with(Arc::clone(&commands), Some(camera), Some(replay));

        let response = service
            .camera_switch_number(Request::new(CameraSwitchNumberRequest {
                car_number: None,
                group: None,
                camera: Some(8),
            }))
            .await
            .expect("partial camera switch should succeed")
            .into_inner();

        assert_eq!(response.car_index, 12);
        assert_eq!(response.group, 6);
        assert_eq!(response.camera, 8);
        assert_eq!(
            commands.sent(),
            vec![BroadcastCommand::CameraSwitchNumber(
                "012".to_string(),
                6,
                8
            )]
        );
    }

    #[tokio::test]
    async fn camera_switch_number_rejects_invalid_proto_fields_before_dependencies() {
        let cases = [
            (
                "car_number",
                CameraSwitchNumberRequest {
                    car_number: Some(String::new()),
                    group: Some(1),
                    camera: Some(1),
                },
            ),
            (
                "group",
                CameraSwitchNumberRequest {
                    car_number: Some("012".to_string()),
                    group: Some(u32::from(u16::MAX) + 1),
                    camera: Some(1),
                },
            ),
            (
                "camera",
                CameraSwitchNumberRequest {
                    car_number: Some("012".to_string()),
                    group: Some(1),
                    camera: Some(u32::from(u16::MAX) + 1),
                },
            ),
        ];

        for (field, request) in cases {
            let commands = Arc::new(FakeCommands::default());
            let service = service_with(Arc::clone(&commands), None, None);

            let error = service
                .camera_switch_number(Request::new(request))
                .await
                .expect_err("invalid request should fail");

            assert_invalid_argument(error, field);
            assert!(
                commands.sent().is_empty(),
                "invalid `{field}` should return before dependency calls"
            );
        }
    }

    #[tokio::test]
    async fn replay_set_play_speed_observes_and_returns_current_state() {
        let commands = Arc::new(FakeCommands::default());
        let camera = Arc::new(FakeCamera::default());
        let replay = Arc::new(
            FakeReplay::default()
                .with_snapshot(Ok(replay_snapshot(0, false)))
                .with_wait(Ok(replay_snapshot(2, true))),
        );
        let service = service_with(Arc::clone(&commands), Some(camera), Some(replay));

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
            commands.sent(),
            vec![BroadcastCommand::ReplaySetPlaySpeed(2, true)]
        );
    }

    #[tokio::test]
    async fn replay_set_play_speed_uses_current_telemetry_for_missing_fields() {
        let commands = Arc::new(FakeCommands::default());
        let camera = Arc::new(FakeCamera::default());
        let replay = Arc::new(
            FakeReplay::default()
                .with_snapshot(Ok(replay_snapshot(4, true)))
                .with_wait(Ok(replay_snapshot(2, true))),
        );
        let service = service_with(Arc::clone(&commands), Some(camera), Some(replay));

        let response = service
            .replay_set_play_speed(Request::new(ReplaySetPlaySpeedRequest {
                speed: Some(2),
                is_slow_motion: None,
            }))
            .await
            .expect("partial replay speed should succeed")
            .into_inner();

        assert_eq!(response.speed, 2);
        assert!(response.is_slow_motion);
        assert_eq!(
            commands.sent(),
            vec![BroadcastCommand::ReplaySetPlaySpeed(2, true)]
        );
    }

    #[tokio::test]
    async fn replay_set_play_speed_rejects_invalid_proto_fields_before_dependencies() {
        let cases = [(
            "speed",
            ReplaySetPlaySpeedRequest {
                speed: Some(i32::from(i16::MAX) + 1),
                is_slow_motion: Some(false),
            },
        )];

        for (field, request) in cases {
            let commands = Arc::new(FakeCommands::default());
            let service = service_with(Arc::clone(&commands), None, None);

            let error = service
                .replay_set_play_speed(Request::new(request))
                .await
                .expect_err("invalid request should fail");

            assert_invalid_argument(error, field);
            assert!(
                commands.sent().is_empty(),
                "invalid `{field}` should return before dependency calls"
            );
        }
    }

    #[tokio::test]
    async fn replay_enum_requests_reject_invalid_values_before_dependencies() {
        let search_cases = [
            ("mode", ReplaySearchRequest { mode: None }),
            (
                "mode",
                ReplaySearchRequest {
                    mode: Some(ReplaySearchMode::Unknown as i32),
                },
            ),
            ("mode", ReplaySearchRequest { mode: Some(999) }),
        ];

        for (field, request) in search_cases {
            let commands = Arc::new(FakeCommands::default());
            let service = service_with(Arc::clone(&commands), None, None);

            let error = service
                .replay_search(Request::new(request))
                .await
                .expect_err("invalid request should fail");

            assert_invalid_argument(error, field);
            assert!(
                commands.sent().is_empty(),
                "invalid `{field}` should return before dependency calls"
            );
        }

        let commands = Arc::new(FakeCommands::default());
        let service = service_with(Arc::clone(&commands), None, None);
        let error = service
            .replay_set_state(Request::new(ReplaySetStateRequest {
                state: Some(ReplayStateMode::Unknown as i32),
            }))
            .await
            .expect_err("unknown state should fail");

        assert_invalid_argument(error, "state");
        assert!(commands.sent().is_empty());
    }

    #[tokio::test]
    async fn chat_command_rejects_invalid_proto_values_before_dependencies() {
        let cases = [
            (
                "mode",
                ChatCommandRequest {
                    mode: None,
                    r#macro: None,
                },
            ),
            (
                "mode",
                ChatCommandRequest {
                    mode: Some(ChatCommandMode::Unknown as i32),
                    r#macro: None,
                },
            ),
            (
                "macro",
                ChatCommandRequest {
                    mode: Some(ChatCommandMode::Macro as i32),
                    r#macro: None,
                },
            ),
            (
                "macro",
                ChatCommandRequest {
                    mode: Some(ChatCommandMode::Macro as i32),
                    r#macro: Some(0),
                },
            ),
            (
                "macro",
                ChatCommandRequest {
                    mode: Some(ChatCommandMode::Macro as i32),
                    r#macro: Some(16),
                },
            ),
        ];

        for (field, request) in cases {
            let commands = Arc::new(FakeCommands::default());
            let service = service_with(Arc::clone(&commands), None, None);

            let error = service
                .chat_command(Request::new(request))
                .await
                .expect_err("invalid request should fail");

            assert_invalid_argument(error, field);
            assert!(
                commands.sent().is_empty(),
                "invalid `{field}` should return before dependency calls"
            );
        }
    }

    #[tokio::test]
    async fn pit_and_force_feedback_reject_invalid_values_before_dependencies() {
        let pit_cases = [
            (
                "mode",
                PitCommandRequest {
                    mode: None,
                    value: None,
                },
            ),
            (
                "value",
                PitCommandRequest {
                    mode: Some(PitCommandMode::Fuel as i32),
                    value: None,
                },
            ),
            (
                "value",
                PitCommandRequest {
                    mode: Some(PitCommandMode::Fuel as i32),
                    value: Some(1.5),
                },
            ),
            (
                "value",
                PitCommandRequest {
                    mode: Some(PitCommandMode::Fuel as i32),
                    value: Some(f32::INFINITY),
                },
            ),
        ];

        for (field, request) in pit_cases {
            let commands = Arc::new(FakeCommands::default());
            let service = service_with(Arc::clone(&commands), None, None);

            let error = service
                .pit_command(Request::new(request))
                .await
                .expect_err("invalid request should fail");

            assert_invalid_argument(error, field);
            assert!(
                commands.sent().is_empty(),
                "invalid `{field}` should return before dependency calls"
            );
        }

        let force_feedback_cases = [
            (
                "mode",
                ForceFeedbackCommandRequest {
                    mode: None,
                    value: Some(20.0),
                },
            ),
            (
                "value",
                ForceFeedbackCommandRequest {
                    mode: Some(ForceFeedbackCommandMode::MaxForce as i32),
                    value: None,
                },
            ),
            (
                "value",
                ForceFeedbackCommandRequest {
                    mode: Some(ForceFeedbackCommandMode::MaxForce as i32),
                    value: Some(f32::NAN),
                },
            ),
        ];

        for (field, request) in force_feedback_cases {
            let commands = Arc::new(FakeCommands::default());
            let service = service_with(Arc::clone(&commands), None, None);

            let error = service
                .force_feedback_command(Request::new(request))
                .await
                .expect_err("invalid request should fail");

            assert_invalid_argument(error, field);
            assert!(
                commands.sent().is_empty(),
                "invalid `{field}` should return before dependency calls"
            );
        }
    }

    #[tokio::test]
    async fn observed_rpc_fails_fast_without_observer() {
        let commands = Arc::new(FakeCommands::default());
        let service = service_with(commands, None, None);

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
        let commands = Arc::new(FakeCommands::default());
        let camera = Arc::new(
            FakeCamera::default()
                .with_snapshot(Ok(camera_snapshot(7, 1, 2, 3)))
                .with_wait(Err(BroadcastError::ObservationTimeout)),
        );
        let replay = Arc::new(FakeReplay::default());
        let service = service_with(commands, Some(camera), Some(replay));

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
    async fn observed_rpc_propagates_source_end_as_unavailable() {
        let commands = Arc::new(FakeCommands::default());
        let camera = Arc::new(
            FakeCamera::default()
                .with_snapshot(Ok(camera_snapshot(7, 1, 2, 3)))
                .with_wait(Err(BroadcastError::ObservationSourceEnded)),
        );
        let replay = Arc::new(FakeReplay::default());
        let service = service_with(commands, Some(camera), Some(replay));

        let error = service
            .camera_switch_position(Request::new(CameraSwitchPositionRequest {
                position: Some(4),
                group: Some(5),
                camera: Some(6),
            }))
            .await
            .expect_err("camera switch should surface source end");

        assert_eq!(error.code(), tonic::Code::Unavailable);
    }

    #[tokio::test]
    async fn command_send_errors_map_to_transport_statuses() {
        let cases = [
            (
                BroadcastError::Sdk(IRacingSDKError::connection_failed("sim disconnected")),
                tonic::Code::Unavailable,
            ),
            (
                BroadcastError::Sdk(IRacingSDKError::unsupported_platform(
                    "live broadcast",
                    "Windows",
                )),
                tonic::Code::FailedPrecondition,
            ),
            (
                BroadcastError::Sdk(IRacingSDKError::Parse {
                    context: "broadcast command".to_string(),
                    details: "unexpected response".to_string(),
                }),
                tonic::Code::Internal,
            ),
        ];

        for (source, expected_code) in cases {
            let commands = Arc::new(FakeCommands::with_error(source));
            let service = service_with(Arc::clone(&commands), None, None);

            let error = service
                .reload_textures(Request::new(ReloadTexturesRequest { car_idx: None }))
                .await
                .expect_err("send failure should map to a transport error");

            assert_eq!(error.code(), expected_code);
            assert_eq!(commands.sent(), vec![BroadcastCommand::ReloadAllTextures]);
        }
    }

    #[tokio::test]
    async fn chat_command_uses_ack_path_without_observation() {
        let commands = Arc::new(FakeCommands::default());
        let service = service_with(Arc::clone(&commands), None, None);

        service
            .chat_command(Request::new(ChatCommandRequest {
                mode: Some(ChatCommandMode::Macro as i32),
                r#macro: Some(3),
            }))
            .await
            .expect("chat command should succeed");

        assert_eq!(commands.sent(), vec![BroadcastCommand::ChatCommandMacro(3)]);
    }
}
