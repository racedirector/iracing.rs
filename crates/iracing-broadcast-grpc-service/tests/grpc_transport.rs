use iracing_broadcast_grpc_service::*;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{
    Request, Response, Status,
    transport::{Channel, Server},
};

#[derive(Debug, Default)]
struct TransportProbe;

#[tonic::async_trait]
impl Broadcast for TransportProbe {
    async fn get_available_cameras(
        &self,
        _request: Request<()>,
    ) -> Result<Response<GetAvailableCamerasResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn camera_switch_position(
        &self,
        request: Request<CameraSwitchPositionRequest>,
    ) -> Result<Response<CameraSwitchPositionResponse>, Status> {
        let request = request.into_inner();

        Ok(Response::new(CameraSwitchPositionResponse {
            car_index: request.position.unwrap_or_default(),
            group: request.group.unwrap_or_default(),
            camera: request.camera.unwrap_or_default(),
        }))
    }

    async fn camera_switch_number(
        &self,
        _request: Request<CameraSwitchNumberRequest>,
    ) -> Result<Response<CameraSwitchNumberResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn camera_set_state(
        &self,
        _request: Request<CameraSetStateRequest>,
    ) -> Result<Response<CameraSetStateResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn replay_set_play_speed(
        &self,
        _request: Request<ReplaySetPlaySpeedRequest>,
    ) -> Result<Response<ReplaySetPlaySpeedResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn replay_set_play_position(
        &self,
        _request: Request<ReplaySetPlayPositionRequest>,
    ) -> Result<Response<ReplaySetPlayPositionResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn replay_search(
        &self,
        _request: Request<ReplaySearchRequest>,
    ) -> Result<Response<ReplaySearchResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn replay_set_state(
        &self,
        _request: Request<ReplaySetStateRequest>,
    ) -> Result<Response<ReplaySetStateResponse>, Status> {
        Err(Status::failed_precondition("transport error probe"))
    }

    async fn reload_textures(
        &self,
        _request: Request<ReloadTexturesRequest>,
    ) -> Result<Response<ReloadTexturesResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn chat_command(
        &self,
        _request: Request<ChatCommandRequest>,
    ) -> Result<Response<ChatCommandResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn pit_command(
        &self,
        _request: Request<PitCommandRequest>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn pit_command_stream(
        &self,
        _request: Request<tonic::Streaming<PitCommandRequest>>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn telemetry_command(
        &self,
        _request: Request<TelemetryCommandRequest>,
    ) -> Result<Response<TelemetryCommandResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn force_feedback_command(
        &self,
        _request: Request<ForceFeedbackCommandRequest>,
    ) -> Result<Response<ForceFeedbackCommandResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn replay_search_session_time(
        &self,
        _request: Request<ReplaySearchSessionTimeRequest>,
    ) -> Result<Response<ReplaySearchSessionTimeResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn video_capture(
        &self,
        _request: Request<VideoCaptureRequest>,
    ) -> Result<Response<VideoCaptureResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }
}

async fn spawn_probe() -> (RawBroadcastClient<Channel>, oneshot::Sender<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("read test listener address");
    let incoming = TcpListenerStream::new(listener);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    tokio::spawn(async move {
        Server::builder()
            .add_service(BroadcastServer::new(TransportProbe))
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("transport probe server should run");
    });

    let client = RawBroadcastClient::connect(format!("http://{addr}"))
        .await
        .expect("connect generated client to test server");

    (client, shutdown_tx)
}

#[tokio::test]
async fn camera_switch_position_round_trips_over_grpc_transport() {
    let (mut client, shutdown) = spawn_probe().await;

    let response = client
        .camera_switch_position(CameraSwitchPositionRequest {
            position: Some(10),
            group: Some(20),
            camera: Some(30),
        })
        .await
        .expect("grpc call should succeed")
        .into_inner();

    assert_eq!(response.car_index, 10);
    assert_eq!(response.group, 20);
    assert_eq!(response.camera, 30);

    let _ = shutdown.send(());
}

#[tokio::test]
async fn tonic_status_crosses_grpc_transport() {
    let (mut client, shutdown) = spawn_probe().await;

    let error = client
        .replay_set_state(ReplaySetStateRequest {
            state: Some(ReplayStateMode::EraseTape as i32),
        })
        .await
        .expect_err("grpc call should return server status");

    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
    assert!(error.message().contains("transport error probe"));

    let _ = shutdown.send(());
}
