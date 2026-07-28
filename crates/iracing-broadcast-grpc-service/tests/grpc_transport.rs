use iracing_broadcast_grpc_service::*;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{
    Request, Response, Status,
    transport::{Channel, Server},
};

#[derive(Debug, Default)]
struct TransportProbe;

fn unimplemented_stream<T>() -> ReceiverStream<Result<T, Status>> {
    let (tx, rx) = mpsc::channel(1);
    tx.try_send(Err(Status::unimplemented("not used by transport test")))
        .expect("unimplemented status should fit in test stream");
    ReceiverStream::new(rx)
}

#[tonic::async_trait]
impl Broadcast for TransportProbe {
    type SubscribeCurrentCameraPositionStream =
        ReceiverStream<Result<CurrentCameraPositionResponse, Status>>;
    type SubscribeCurrentCameraStateStream =
        ReceiverStream<Result<CurrentCameraStateResponse, Status>>;
    type SubscribeCurrentReplayPlaySpeedStream =
        ReceiverStream<Result<CurrentReplayPlaySpeedResponse, Status>>;
    type SubscribeCurrentReplayPositionStream =
        ReceiverStream<Result<CurrentReplayPositionResponse, Status>>;
    type SubscribeCurrentPitServiceStream =
        ReceiverStream<Result<CurrentPitServiceResponse, Status>>;
    type SubscribeCurrentTelemetryStateStream =
        ReceiverStream<Result<CurrentTelemetryStateResponse, Status>>;
    type SubscribeCurrentForceFeedbackStream =
        ReceiverStream<Result<CurrentForceFeedbackResponse, Status>>;
    type SubscribeCurrentVideoCaptureStream =
        ReceiverStream<Result<CurrentVideoCaptureResponse, Status>>;

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

    async fn current_camera_position(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CurrentCameraPositionResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn subscribe_current_camera_position(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentCameraPositionStream>, Status> {
        let (tx, rx) = mpsc::channel(2);
        tx.send(Ok(CurrentCameraPositionResponse {
            car_index: 10,
            group: 20,
            camera: 30,
        }))
        .await
        .expect("first update should fit in test stream");
        tx.send(Ok(CurrentCameraPositionResponse {
            car_index: 11,
            group: 21,
            camera: 31,
        }))
        .await
        .expect("second update should fit in test stream");

        Ok(Response::new(ReceiverStream::new(rx)))
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

    async fn current_camera_state(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CurrentCameraStateResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn subscribe_current_camera_state(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentCameraStateStream>, Status> {
        Ok(Response::new(unimplemented_stream()))
    }

    async fn replay_set_play_speed(
        &self,
        _request: Request<ReplaySetPlaySpeedRequest>,
    ) -> Result<Response<ReplaySetPlaySpeedResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn current_replay_play_speed(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CurrentReplayPlaySpeedResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn subscribe_current_replay_play_speed(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentReplayPlaySpeedStream>, Status> {
        Ok(Response::new(unimplemented_stream()))
    }

    async fn replay_set_play_position(
        &self,
        _request: Request<ReplaySetPlayPositionRequest>,
    ) -> Result<Response<ReplaySetPlayPositionResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn current_replay_position(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CurrentReplayPositionResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn subscribe_current_replay_position(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentReplayPositionStream>, Status> {
        Ok(Response::new(unimplemented_stream()))
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

    async fn current_pit_service(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CurrentPitServiceResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn subscribe_current_pit_service(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentPitServiceStream>, Status> {
        Ok(Response::new(unimplemented_stream()))
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

    async fn current_telemetry_state(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CurrentTelemetryStateResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn subscribe_current_telemetry_state(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentTelemetryStateStream>, Status> {
        Ok(Response::new(unimplemented_stream()))
    }

    async fn force_feedback_command(
        &self,
        _request: Request<ForceFeedbackCommandRequest>,
    ) -> Result<Response<ForceFeedbackCommandResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn current_force_feedback(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CurrentForceFeedbackResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn subscribe_current_force_feedback(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentForceFeedbackStream>, Status> {
        Ok(Response::new(unimplemented_stream()))
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

    async fn current_video_capture(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CurrentVideoCaptureResponse>, Status> {
        Err(Status::unimplemented("not used by transport test"))
    }

    async fn subscribe_current_video_capture(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::SubscribeCurrentVideoCaptureStream>, Status> {
        Ok(Response::new(unimplemented_stream()))
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

#[tokio::test]
async fn server_streaming_updates_cross_grpc_transport() {
    let (mut client, shutdown) = spawn_probe().await;

    let mut stream = client
        .subscribe_current_camera_position(())
        .await
        .expect("subscription should start")
        .into_inner();

    let first = stream
        .message()
        .await
        .expect("first stream item should decode")
        .expect("first stream item should exist");
    assert_eq!(first.car_index, 10);
    assert_eq!(first.group, 20);
    assert_eq!(first.camera, 30);

    let second = stream
        .message()
        .await
        .expect("second stream item should decode")
        .expect("second stream item should exist");
    assert_eq!(second.car_index, 11);
    assert_eq!(second.group, 21);
    assert_eq!(second.camera, 31);

    assert!(
        stream
            .message()
            .await
            .expect("stream end should decode")
            .is_none()
    );

    let _ = shutdown.send(());
}
