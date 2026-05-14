use std::{sync::Arc, thread, time::Duration};

use tonic::{Request, Response, Status, transport::Server};

use broadcast::broadcast_server::{Broadcast, BroadcastServer};
use broadcast::*;

use iracing_sdk::{
    Broadcast as BroadcastClient, BroadcastCommand, FramePacket, IRacingSDKError, LiveProvider,
    Provider, VarData, VariableSchema,
};
use tokio::sync::{Mutex, watch};

pub mod broadcast {
    tonic::include_proto!("iracing.broadcast");
}

const DEFAULT_TELEMETRY_TIMEOUT_MS: u64 = 750;

/// The latest telemetry frame seen by the background observer.
///
/// This deliberately keeps the raw `FramePacket`; RPCs can experiment with field
/// names and decoding rules locally while the schema is still moving.
#[derive(Debug, Default)]
struct TelemetrySnapshot {
    sequence: u64,
    frame: Option<FramePacket>,
}

impl TelemetrySnapshot {
    fn sequence(&self) -> u64 {
        self.sequence
    }

    fn has_frame(&self) -> bool {
        self.frame.is_some()
    }

    fn i32(&self, field: &str) -> Option<i32> {
        let frame = self.frame.as_ref()?;
        let variable = frame.schema.get_variable(field)?;

        i32::from_bytes(frame.data.as_ref(), variable).ok()
    }
}

/// Passive telemetry reader shared by RPC handlers.
///
/// The observer owns `LiveProvider` on a dedicated current-thread runtime because
/// the provider trait is intentionally `?Send`. It stores the connection schema
/// once, then publishes raw frame snapshots through a `watch` channel.
#[derive(Debug, Clone)]
struct TelemetryObserver {
    schema: Arc<VariableSchema>,
    rx: watch::Receiver<Arc<TelemetrySnapshot>>,
}

impl TelemetryObserver {
    fn start(provider: LiveProvider) -> TelemetryObserver {
        let schema = provider.schema();
        let (tx, rx) = watch::channel(Arc::new(TelemetrySnapshot::default()));

        thread::Builder::new()
            .name("iracing-telemetry-observer".to_string())
            .spawn(move || Self::run_provider(provider, tx))
            .expect("failed to spawn telemetry observer thread");

        TelemetryObserver { schema, rx }
    }

    fn snapshot(&self) -> Arc<TelemetrySnapshot> {
        self.rx.borrow().clone()
    }

    fn has_fields(&self, fields: &[&str]) -> bool {
        fields
            .iter()
            .all(|field| self.schema.get_variable(field).is_some())
    }

    async fn wait_for(
        &self,
        timeout: Duration,
        mut predicate: impl FnMut(&TelemetrySnapshot) -> bool + Send + 'static,
    ) -> Result<Arc<TelemetrySnapshot>, Status> {
        let mut rx = self.rx.clone();

        let wait = async move {
            loop {
                let snapshot = rx.borrow_and_update().clone();
                if predicate(&snapshot) {
                    return Ok(snapshot);
                }

                rx.changed()
                    .await
                    .map_err(|_| Status::unavailable("telemetry observer stopped"))?;
            }
        };

        tokio::time::timeout(timeout, wait)
            .await
            .map_err(|_| Status::deadline_exceeded("camera switch was not observed in telemetry"))?
    }

    async fn observe(mut provider: LiveProvider, tx: watch::Sender<Arc<TelemetrySnapshot>>) {
        let mut sequence = 0u64;

        loop {
            match provider.next_frame().await {
                Ok(Some(frame)) => {
                    sequence = sequence.saturating_add(1);

                    if tx
                        .send(Arc::new(TelemetrySnapshot {
                            sequence,
                            frame: Some(frame),
                        }))
                        .is_err()
                    {
                        tracing::debug!("telemetry observer has no receivers; stopping");
                        break;
                    }
                }
                Ok(None) => {
                    tracing::warn!("live telemetry provider ended");
                    break;
                }
                Err(error) => {
                    tracing::warn!(error = %error, "live telemetry observer failed");
                    break;
                }
            }
        }
    }

    fn run_provider(provider: LiveProvider, tx: watch::Sender<Arc<TelemetrySnapshot>>) {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::error!(error = %error, "failed to start telemetry observer runtime");
                return;
            }
        };

        runtime.block_on(Self::observe(provider, tx));
    }
}

/// Minimal telemetry projection needed by `CameraSwitchPosition`.
///
/// This is intentionally validation-lite: the field names are declared in one
/// place, and decoding happens only when this RPC needs it.
#[derive(Debug, Clone, Copy)]
struct CameraSwitchPositionTelemetry {
    car_index: i32,
    group: i32,
    camera: i32,
}

impl CameraSwitchPositionTelemetry {
    const CAR_INDEX: &'static str = "CamCarIdx";
    const GROUP: &'static str = "CamGroupNumber";
    const CAMERA: &'static str = "CamCameraNumber";
    const FIELDS: &'static [&'static str] = &[Self::CAR_INDEX, Self::GROUP, Self::CAMERA];

    fn from_snapshot(snapshot: &TelemetrySnapshot) -> Option<Self> {
        Some(Self {
            car_index: snapshot.i32(Self::CAR_INDEX)?,
            group: snapshot.i32(Self::GROUP)?,
            camera: snapshot.i32(Self::CAMERA)?,
        })
    }

    fn matches_request(&self, position: u16, group: u16, camera: u16) -> bool {
        self.car_index == i32::from(position)
            && self.group == i32::from(group)
            && self.camera == i32::from(camera)
    }
}

#[derive(Debug, Default)]
pub struct BroadcastServiceBuilder {
    client: Option<BroadcastClient>,
    provider: Option<LiveProvider>,
    telemetry_timeout: Option<Duration>,
}

impl BroadcastServiceBuilder {
    pub fn with_client(mut self, client: BroadcastClient) -> Self {
        self.client = Some(client);
        self
    }

    pub fn with_provider(mut self, provider: LiveProvider) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn with_telemetry_timeout(mut self, timeout: Duration) -> Self {
        self.telemetry_timeout = Some(timeout);
        self
    }

    pub fn build(self) -> Result<BroadcastService, IRacingSDKError> {
        let client = match self.client {
            Some(c) => c,
            None => BroadcastClient::new()?,
        };

        let provider = match self.provider {
            Some(p) => p,
            None => LiveProvider::new()?,
        };

        let telemetry = TelemetryObserver::start(provider);

        let telemetry_timeout = match self.telemetry_timeout {
            Some(t) => t,
            None => Duration::from_millis(DEFAULT_TELEMETRY_TIMEOUT_MS),
        };

        Ok(BroadcastService {
            client,
            telemetry,
            command_lock: Mutex::new(()),
            telemetry_timeout,
        })
    }
}

#[derive(Debug)]
pub struct BroadcastService {
    client: BroadcastClient,
    telemetry: TelemetryObserver,
    command_lock: Mutex<()>,

    telemetry_timeout: Duration,
}

impl BroadcastService {
    pub fn builder() -> BroadcastServiceBuilder {
        BroadcastServiceBuilder::default()
    }

    pub fn new() -> Result<Self, Status> {
        Self::builder()
            .build()
            .map_err(Self::broadcast_error_to_status)
    }

    fn unimplemented_response<T>(method: &str) -> Result<Response<T>, Status> {
        tracing::info!(method, "broadcast RPC not implemented");

        Err(Status::unimplemented(format!(
            "{method} is not yet implemented"
        )))
    }

    fn broadcast_error_to_status(error: IRacingSDKError) -> Status {
        let message = error.to_string();
        let retryable = error.is_retryable();

        tracing::warn!(
            error = %error,
            retryable,
            "iRacing broadcast operation failed"
        );

        match &error {
            IRacingSDKError::Connection { .. } => Status::unavailable(message),
            IRacingSDKError::UnsupportedPlatform { .. } => Status::failed_precondition(message),
            #[cfg(windows)]
            IRacingSDKError::WindowsApi { .. } => Status::unavailable(message),
            IRacingSDKError::Buffer { .. } if retryable => Status::unavailable(message),
            _ if retryable => Status::unavailable(message),
            _ => Status::internal(message),
        }
    }

    fn send_message(&self, message: BroadcastCommand) -> Result<(), Status> {
        self.client
            .send_message(message.into())
            .map_err(Self::broadcast_error_to_status)
    }

    fn proto_u32_to_u16(field_name: &'static str, value: u32) -> Result<u16, Status> {
        u16::try_from(value).map_err(|_| {
            Status::invalid_argument(format!(
                "{field_name} must be in the range 0..={}, got {value}",
                u16::MAX,
            ))
        })
    }

    fn required_proto_u16(field_name: &'static str, value: Option<u32>) -> Result<u16, Status> {
        match value {
            Some(value) => Self::proto_u32_to_u16(field_name, value),
            None => Err(Status::invalid_argument(format!("Missing `{field_name}`"))),
        }
    }
}

#[tonic::async_trait]
impl Broadcast for BroadcastService {
    async fn get_available_cameras(
        &self,
        _request: Request<()>,
    ) -> Result<Response<GetAvailableCamerasResponse>, Status> {
        Self::unimplemented_response("get_available_cameras")
    }

    async fn camera_switch_position(
        &self,
        request: Request<CameraSwitchPositionRequest>,
    ) -> Result<Response<CameraSwitchPositionResponse>, Status> {
        let CameraSwitchPositionRequest {
            position,
            group,
            camera,
        } = request.into_inner();

        let position = Self::required_proto_u16("position", position)?;
        let group = Self::required_proto_u16("group", group)?;
        let camera = Self::required_proto_u16("camera", camera)?;

        let _command_guard = self.command_lock.lock().await;
        let before = self.telemetry.snapshot();

        if !before.has_frame() {
            return Err(Status::unavailable(
                "no live telemetry frame is available yet",
            ));
        }

        if !self
            .telemetry
            .has_fields(CameraSwitchPositionTelemetry::FIELDS)
        {
            return Err(Status::failed_precondition(
                "live telemetry does not expose the camera switch fields",
            ));
        }

        let before_sequence = before.sequence();

        self.send_message(BroadcastCommand::CameraSwitchPosition(
            position, group, camera,
        ))?;

        let snapshot = self
            .telemetry
            .wait_for(self.telemetry_timeout, move |snapshot| {
                snapshot.sequence() > before_sequence
                    && CameraSwitchPositionTelemetry::from_snapshot(snapshot)
                        .is_some_and(|telemetry| telemetry.matches_request(position, group, camera))
            })
            .await?;

        let telemetry = CameraSwitchPositionTelemetry::from_snapshot(&snapshot)
            .ok_or_else(|| Status::unavailable("camera telemetry observer stopped"))?;

        Ok(Response::new(CameraSwitchPositionResponse {
            car_index: telemetry.car_index as u32,
            group: telemetry.group as u32,
            camera: telemetry.camera as u32,
        }))
    }

    async fn camera_switch_number(
        &self,
        _request: Request<CameraSwitchNumberRequest>,
    ) -> Result<Response<CameraSwitchNumberResponse>, Status> {
        Self::unimplemented_response("camera_switch_number")
    }

    async fn camera_set_state(
        &self,
        _request: Request<CameraSetStateRequest>,
    ) -> Result<Response<CameraSetStateResponse>, Status> {
        Self::unimplemented_response("camera_set_state")
    }

    async fn replay_set_play_speed(
        &self,
        _request: Request<ReplaySetPlaySpeedRequest>,
    ) -> Result<Response<ReplaySetPlaySpeedResponse>, Status> {
        Self::unimplemented_response("replay_set_play_speed")
    }

    async fn replay_set_play_position(
        &self,
        _request: Request<ReplaySetPlayPositionRequest>,
    ) -> Result<Response<ReplaySetPlayPositionResponse>, Status> {
        Self::unimplemented_response("replay_set_play_position")
    }

    async fn replay_search(
        &self,
        _request: Request<ReplaySearchRequest>,
    ) -> Result<Response<ReplaySearchResponse>, Status> {
        Self::unimplemented_response("replay_search")
    }

    async fn replay_set_state(
        &self,
        _request: Request<ReplaySetStateRequest>,
    ) -> Result<Response<ReplaySetStateResponse>, Status> {
        Self::unimplemented_response("replay_set_state")
    }

    async fn reload_textures(
        &self,
        request: Request<ReloadTexturesRequest>,
    ) -> Result<Response<ReloadTexturesResponse>, Status> {
        let ReloadTexturesRequest { car_idx } = request.into_inner();

        self.send_message(match car_idx {
            Some(index) => Self::proto_u32_to_u16("car_idx", index)
                .map(|i| BroadcastCommand::ReloadTextures(i))?,
            None => BroadcastCommand::ReloadAllTextures,
        })
        .map(|_| Response::new(ReloadTexturesResponse {}))
    }

    async fn chat_command(
        &self,
        _request: Request<ChatCommandRequest>,
    ) -> Result<Response<ChatCommandResponse>, Status> {
        Self::unimplemented_response("chat_command")
    }

    async fn pit_command(
        &self,
        _request: Request<PitCommandRequest>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        Self::unimplemented_response("pit_command")
    }

    async fn pit_command_stream(
        &self,
        _request: Request<tonic::Streaming<PitCommandRequest>>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        Self::unimplemented_response("pit_command_stream")
    }

    async fn telemetry_command(
        &self,
        _request: Request<TelemetryCommandRequest>,
    ) -> Result<Response<TelemetryCommandResponse>, Status> {
        Self::unimplemented_response("telemetry_command")
    }

    async fn force_feedback_command(
        &self,
        _request: Request<ForceFeedbackCommandRequest>,
    ) -> Result<Response<ForceFeedbackCommandResponse>, Status> {
        Self::unimplemented_response("force_feedback_command")
    }

    async fn replay_search_session_time(
        &self,
        _request: Request<ReplaySearchSessionTimeRequest>,
    ) -> Result<Response<ReplaySearchSessionTimeResponse>, Status> {
        Self::unimplemented_response("replay_search_session_time")
    }

    async fn video_capture(
        &self,
        _request: Request<VideoCaptureRequest>,
    ) -> Result<Response<VideoCaptureResponse>, Status> {
        Self::unimplemented_response("video_capture")
    }
}
