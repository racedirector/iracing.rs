use tonic::{Request, Response, Status, transport::Server};

use broadcast::broadcast_server::{Broadcast, BroadcastServer};
use broadcast::*;

pub mod broadcast {
    tonic::include_proto!("iracing.broadcast");
}

#[derive(Debug, Default)]
pub struct BroadcastService {}

fn unimplemented_response<T>(method: &str) -> Result<Response<T>, Status> {
    tracing::info!(method, "broadcast RPC not implemented");

    Err(Status::unimplemented(format!(
        "{method} is not yet implemented"
    )))
}

#[tonic::async_trait]
impl Broadcast for BroadcastService {
    async fn get_available_cameras(
        &self,
        _request: Request<()>,
    ) -> Result<Response<GetAvailableCamerasResponse>, Status> {
        unimplemented_response("get_available_cameras")
    }

    async fn camera_switch_position(
        &self,
        _request: Request<CameraSwitchPositionRequest>,
    ) -> Result<Response<CameraSwitchPositionResponse>, Status> {
        unimplemented_response("camera_switch_position")
    }

    async fn camera_switch_number(
        &self,
        _request: Request<CameraSwitchNumberRequest>,
    ) -> Result<Response<CameraSwitchNumberResponse>, Status> {
        unimplemented_response("camera_switch_number")
    }

    async fn camera_set_state(
        &self,
        _request: Request<CameraSetStateRequest>,
    ) -> Result<Response<CameraSetStateResponse>, Status> {
        unimplemented_response("camera_set_state")
    }

    async fn replay_set_play_speed(
        &self,
        _request: Request<ReplaySetPlaySpeedRequest>,
    ) -> Result<Response<ReplaySetPlaySpeedResponse>, Status> {
        unimplemented_response("replay_set_play_speed")
    }

    async fn replay_set_play_position(
        &self,
        _request: Request<ReplaySetPlayPositionRequest>,
    ) -> Result<Response<ReplaySetPlayPositionResponse>, Status> {
        unimplemented_response("replay_set_play_position")
    }

    async fn replay_search(
        &self,
        _request: Request<ReplaySearchRequest>,
    ) -> Result<Response<ReplaySearchResponse>, Status> {
        unimplemented_response("replay_search")
    }

    async fn replay_set_state(
        &self,
        _request: Request<ReplaySetStateRequest>,
    ) -> Result<Response<ReplaySetStateResponse>, Status> {
        unimplemented_response("replay_set_state")
    }

    async fn reload_textures(
        &self,
        _request: Request<ReloadTexturesRequest>,
    ) -> Result<Response<ReloadTexturesResponse>, Status> {
        unimplemented_response("reload_textures")
    }

    async fn chat_command(
        &self,
        _request: Request<ChatCommandRequest>,
    ) -> Result<Response<ChatCommandResponse>, Status> {
        unimplemented_response("chat_command")
    }

    async fn pit_command(
        &self,
        _request: Request<PitCommandRequest>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        unimplemented_response("pit_command")
    }

    async fn pit_command_stream(
        &self,
        _request: Request<tonic::Streaming<PitCommandRequest>>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        unimplemented_response("pit_command_stream")
    }

    async fn telemetry_command(
        &self,
        _request: Request<TelemetryCommandRequest>,
    ) -> Result<Response<TelemetryCommandResponse>, Status> {
        unimplemented_response("telemetry_command")
    }

    async fn force_feedback_command(
        &self,
        _request: Request<ForceFeedbackCommandRequest>,
    ) -> Result<Response<ForceFeedbackCommandResponse>, Status> {
        unimplemented_response("force_feedback_command")
    }

    async fn replay_search_session_time(
        &self,
        _request: Request<ReplaySearchSessionTimeRequest>,
    ) -> Result<Response<ReplaySearchSessionTimeResponse>, Status> {
        unimplemented_response("replay_search_session_time")
    }

    async fn video_capture(
        &self,
        _request: Request<VideoCaptureRequest>,
    ) -> Result<Response<VideoCaptureResponse>, Status> {
        unimplemented_response("video_capture")
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let broadcast = BroadcastService::default();

    Server::builder()
        .add_service(BroadcastServer::new(broadcast))
        .serve(addr)
        .await?;

    Ok(())
}
