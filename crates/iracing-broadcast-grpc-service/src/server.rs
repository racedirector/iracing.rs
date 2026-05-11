use tonic::{Request, Response, Status, transport::Server};

use broadcast::broadcast_server::{Broadcast, BroadcastServer};
use broadcast::*;

use iracing_sdk::{
     Broadcast as BroadcastClient, BroadcastCommand, 
    IRacingSDKError,  LiveProvider,
};

pub mod broadcast {
    tonic::include_proto!("iracing.broadcast");
}

fn proto_u32_to_u16(field_name: &'static str, value: u32) -> Result<u16, Status> {
    u16::try_from(value).map_err(|_| {
        Status::invalid_argument(format!(
            "{field_name} must be in the range 0..={}, got {value}",
            u16::MAX,
        ))
    })
}

fn proto_option_u32_to_u16(field_name: &'static str, value: Option<u32>) -> Result<u16, Status> {
    match value {
        Some(v) => proto_u32_to_u16(field_name, v),
        None => Err(Status::invalid_argument(format!("Missing `{field_name}`"))),
    }
}

#[derive(Default)]
struct BroadcastServiceBuilder {
    client: Option<BroadcastClient>,
    provider: Option<LiveProvider>,
}

impl BroadcastServiceBuilder {
    fn with_client(mut self, client: BroadcastClient) -> BroadcastServiceBuilder {
        self.client = Some(client);
        self
    }

    fn with_provider(mut self, provider: LiveProvider) -> BroadcastServiceBuilder {
        self.provider = Some(provider);
        self
    }

    fn build(self) -> Result<BroadcastService, IRacingSDKError> {
        let client = match self.client {
            Some(c) => c,
            None => BroadcastClient::new()?,
        };

        let provider = match self.provider {
            Some(p) => p,
            None => LiveProvider::new()?
        }

        Ok(BroadcastService { client })
    }
}

#[derive(Debug)]
pub struct BroadcastService {
    client: BroadcastClient,
}

impl BroadcastService {
    fn builder() -> BroadcastServiceBuilder {
        BroadcastServiceBuilder::default()
    }

    fn new() -> Result<Self, Status> {
        Self::builder().build().map_err(Self::broadcast_error_to_status)
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

        let position = proto_option_u32_to_u16("position", position)?;
        let group = proto_option_u32_to_u16("group", group)?;
        let camera = proto_option_u32_to_u16("camera", camera)?;

        // TODO: Get the current `CamCarIdx` from telemetry stream

        self.send_message(BroadcastCommand::CameraSwitchPosition(
            position, group, camera,
        ))?;

        // TODO: Wait until `CamCarIdx` and associated telemetry values change

        // TODO: Respond successfully:
        // Response::new(CameraSwitchPositionResponse {
        //     position,
        //     group,
        //     camera,
        // })

        // TODO: Or handle a timeout/failure to update and respond with error status.

        Self::unimplemented_response("camera_switch_position")
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
            Some(index) => {
                proto_u32_to_u16("car_idx", index).map(|i| BroadcastCommand::ReloadTextures(i))?
            }
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let broadcast = BroadcastService::new()?;

    Server::builder()
        .add_service(BroadcastServer::new(broadcast))
        .serve(addr)
        .await?;

    Ok(())
}
