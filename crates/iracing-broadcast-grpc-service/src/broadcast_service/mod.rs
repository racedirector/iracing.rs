mod builder;
mod command;
mod error;
mod request;
mod response;

use tonic::{Request, Response, Status};

use crate::broadcast::broadcast_server::Broadcast;
use crate::broadcast::*;

use iracing_sdk::{Broadcast as BroadcastClient, BroadcastCommand};

pub use builder::BroadcastServiceBuilder;
use command as command_impl;
use error::broadcast_error_to_status;
use request as request_impl;
use response as response_impl;

#[derive(Debug)]
pub struct BroadcastService {
    pub(crate) client: BroadcastClient,
}

impl BroadcastService {
    pub fn builder() -> crate::BroadcastServiceBuilder {
        crate::BroadcastServiceBuilder::default()
    }

    pub fn new() -> Result<Self, Status> {
        Self::builder().build().map_err(broadcast_error_to_status)
    }

    fn send_message(&self, message: BroadcastCommand) -> Result<(), Status> {
        self.client
            .send_message(message)
            .map_err(broadcast_error_to_status)
    }
}

#[tonic::async_trait]
impl Broadcast for BroadcastService {
    async fn get_available_cameras(
        &self,
        _request: Request<()>,
    ) -> Result<Response<GetAvailableCamerasResponse>, Status> {
        // TODO: Read camera groups/cameras from session data.
        // TODO: Return the current camera once a shared response pattern exists.
        Ok(Response::new(GetAvailableCamerasResponse {
            camera_groups: Vec::new(),
            car_index: 0,
            group: 0,
            camera: 0,
        }))
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

        let position = request_impl::required_u16("position", position)?;
        let group = request_impl::required_u16("group", group)?;
        let camera = request_impl::required_u16("camera", camera)?;

        // TODO: Get previous camera position.

        self.send_message(BroadcastCommand::CameraSwitchPosition(
            position, group, camera,
        ))?;

        // TODO: Wait for camera position to change.
        // TODO: Resolve the selected position back to a car index.

        Ok(Response::new(CameraSwitchPositionResponse {
            car_index: u32::from(position),
            group: u32::from(group),
            camera: u32::from(camera),
        }))
    }

    async fn camera_switch_number(
        &self,
        request: Request<CameraSwitchNumberRequest>,
    ) -> Result<Response<CameraSwitchNumberResponse>, Status> {
        let CameraSwitchNumberRequest {
            car_number,
            group,
            camera,
        } = request.into_inner();

        let car_number = request_impl::required_string("car_number", car_number)?;
        let group = request_impl::required_u16("group", group)?;
        let camera = request_impl::required_u16("camera", camera)?;

        // TODO: Get previous camera position.

        self.send_message(BroadcastCommand::CameraSwitchNumber(
            car_number, group, camera,
        ))?;

        // TODO: Wait for camera position to change.
        // TODO: Resolve the selected car number back to a car index.

        Ok(Response::new(CameraSwitchNumberResponse {
            car_index: 0,
            group: u32::from(group),
            camera: u32::from(camera),
        }))
    }

    async fn camera_set_state(
        &self,
        request: Request<CameraSetStateRequest>,
    ) -> Result<Response<CameraSetStateResponse>, Status> {
        let CameraSetStateRequest { state } = request.into_inner();

        let state = state.ok_or_else(|| Status::invalid_argument("Missing `state`"))?;

        // TODO: Get previous state

        self.send_message(BroadcastCommand::CameraSetState(
            iracing_sdk::CameraState::from_bits_retain(state),
        ))?;

        // TODO: Wait for state to change

        Ok(Response::new(CameraSetStateResponse { state }))
    }

    async fn replay_set_play_speed(
        &self,
        request: Request<ReplaySetPlaySpeedRequest>,
    ) -> Result<Response<ReplaySetPlaySpeedResponse>, Status> {
        let ReplaySetPlaySpeedRequest {
            speed,
            is_slow_motion,
        } = request.into_inner();

        let speed = request_impl::required_i16("speed", speed)?;
        let is_slow_motion = request_impl::required_bool("is_slow_motion", is_slow_motion)?;

        // TODO: Get previous replay play speed.

        self.send_message(BroadcastCommand::ReplaySetPlaySpeed(speed, is_slow_motion))?;

        // TODO: Wait for replay play speed to change.

        Ok(Response::new(ReplaySetPlaySpeedResponse {
            speed: i32::from(speed),
            is_slow_motion,
        }))
    }

    async fn replay_set_play_position(
        &self,
        request: Request<ReplaySetPlayPositionRequest>,
    ) -> Result<Response<ReplaySetPlayPositionResponse>, Status> {
        let ReplaySetPlayPositionRequest { mode, frame } = request.into_inner();

        let mode = request_impl::required_enum::<ReplayPositionMode>("mode", mode)?;
        let frame = frame.ok_or_else(|| Status::invalid_argument("Missing `frame`"))?;

        // TODO: Get previous replay frame.

        self.send_message(BroadcastCommand::ReplaySetPlayPosition(
            command_impl::replay_position_mode(mode),
            frame,
        ))?;

        // TODO: Wait for replay frame to change.

        Ok(Response::new(ReplaySetPlayPositionResponse { frame }))
    }

    async fn replay_search(
        &self,
        request: Request<ReplaySearchRequest>,
    ) -> Result<Response<ReplaySearchResponse>, Status> {
        let ReplaySearchRequest { mode } = request.into_inner();

        let mode = request_impl::required_enum::<ReplaySearchMode>("mode", mode)?;

        // TODO: Get previous replay frame/session position.

        self.send_message(BroadcastCommand::ReplaySearch(
            command_impl::replay_search_mode(mode),
        ))?;

        // TODO: Wait for replay search result and return frame/session/time.

        Ok(Response::new(ReplaySearchResponse {
            frame: 0,
            session_number: 0,
            session_time: 0.0,
        }))
    }

    async fn replay_set_state(
        &self,
        request: Request<ReplaySetStateRequest>,
    ) -> Result<Response<ReplaySetStateResponse>, Status> {
        let ReplaySetStateRequest { state } = request.into_inner();

        let state = request_impl::required_enum::<ReplayStateMode>("state", state)?;

        // TODO: Get previous replay state.

        self.send_message(BroadcastCommand::ReplaySetState(
            command_impl::replay_state_mode(state),
        ))?;

        // TODO: Wait for replay state to change.

        Ok(Response::new(ReplaySetStateResponse {}))
    }

    async fn reload_textures(
        &self,
        request: Request<ReloadTexturesRequest>,
    ) -> Result<Response<ReloadTexturesResponse>, Status> {
        let ReloadTexturesRequest { car_idx } = request.into_inner();

        self.send_message(match car_idx {
            Some(index) => {
                request_impl::u32_to_u16("car_idx", index).map(BroadcastCommand::ReloadTextures)?
            }
            None => BroadcastCommand::ReloadAllTextures,
        })
        .map(|_| Response::new(ReloadTexturesResponse {}))
    }

    async fn chat_command(
        &self,
        request: Request<ChatCommandRequest>,
    ) -> Result<Response<ChatCommandResponse>, Status> {
        // TODO: Get previous chat state if iRacing exposes one.

        self.send_message(command_impl::chat_command(request.into_inner())?)?;

        // TODO: Wait for chat command acknowledgement/state if available.

        Ok(Response::new(ChatCommandResponse {}))
    }

    async fn pit_command(
        &self,
        request: Request<PitCommandRequest>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        let command = command_impl::pit_command(request.into_inner())?;

        // TODO: Get previous pit service state.

        self.send_message(BroadcastCommand::PitCommand(command))?;

        // TODO: Wait for pit service state to change and return selected values.

        Ok(Response::new(response_impl::empty_pit_command_response()))
    }

    async fn pit_command_stream(
        &self,
        request: Request<tonic::Streaming<PitCommandRequest>>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        let mut stream = request.into_inner();

        // TODO: Get previous pit service state.

        while let Some(request) = stream.message().await? {
            let command = command_impl::pit_command(request)?;
            self.send_message(BroadcastCommand::PitCommand(command))?;
        }

        // TODO: Wait for pit service state to change and return selected values.

        Ok(Response::new(response_impl::empty_pit_command_response()))
    }

    async fn telemetry_command(
        &self,
        request: Request<TelemetryCommandRequest>,
    ) -> Result<Response<TelemetryCommandResponse>, Status> {
        let TelemetryCommandRequest { mode } = request.into_inner();

        let mode = request_impl::required_enum::<TelemetryCommandMode>("mode", mode)?;

        // TODO: Get previous disk telemetry logging state.

        self.send_message(BroadcastCommand::TelemetryCommand(
            command_impl::telemetry_command_mode(mode),
        ))?;

        // TODO: Wait for disk telemetry logging state to change.

        Ok(Response::new(TelemetryCommandResponse {
            is_disk_logging_enabled: false,
            is_disk_logging_active: false,
        }))
    }

    async fn force_feedback_command(
        &self,
        request: Request<ForceFeedbackCommandRequest>,
    ) -> Result<Response<ForceFeedbackCommandResponse>, Status> {
        let ForceFeedbackCommandRequest { mode, value } = request.into_inner();

        let mode = request_impl::required_enum::<ForceFeedbackCommandMode>("mode", mode)?;
        let value = request_impl::required_f32("value", value)?;

        match mode {
            ForceFeedbackCommandMode::MaxForce => {
                // TODO: Get previous force feedback max force.

                self.send_message(BroadcastCommand::FFBCommand(value))?;

                // TODO: Wait for force feedback max force to change.

                Ok(Response::new(ForceFeedbackCommandResponse {
                    max_force: value,
                }))
            }
            ForceFeedbackCommandMode::Unknown => {
                unreachable!("unknown force feedback command mode is rejected")
            }
        }
    }

    async fn replay_search_session_time(
        &self,
        request: Request<ReplaySearchSessionTimeRequest>,
    ) -> Result<Response<ReplaySearchSessionTimeResponse>, Status> {
        let ReplaySearchSessionTimeRequest {
            session_number,
            session_time_ms,
        } = request.into_inner();

        let session_number = request_impl::required_u16("session_number", session_number)?;
        let session_time_ms =
            session_time_ms.ok_or_else(|| Status::invalid_argument("Missing `session_time_ms`"))?;

        // TODO: Get previous replay frame/session position.

        self.send_message(BroadcastCommand::ReplaySearchSessionTime(
            session_number,
            session_time_ms,
        ))?;

        // TODO: Wait for replay search result.

        Ok(Response::new(ReplaySearchSessionTimeResponse {}))
    }

    async fn video_capture(
        &self,
        request: Request<VideoCaptureRequest>,
    ) -> Result<Response<VideoCaptureResponse>, Status> {
        let VideoCaptureRequest { mode } = request.into_inner();

        let mode = request_impl::required_enum::<VideoCaptureMode>("mode", mode)?;

        // TODO: Get previous video capture state if iRacing exposes one.

        self.send_message(BroadcastCommand::VideoCapture(
            command_impl::video_capture_mode(mode),
        ))?;

        // TODO: Wait for video capture state/acknowledgement if available.

        Ok(Response::new(VideoCaptureResponse {}))
    }
}
