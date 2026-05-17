use tonic::{Request, Response, Status};

use crate::broadcast::broadcast_server::Broadcast;
use crate::broadcast::*;

use iracing_sdk::{Broadcast as BroadcastClient, BroadcastCommand, IRacingSDKError, PitCommand};

#[derive(Debug, Default)]
pub struct BroadcastServiceBuilder {
    client: Option<BroadcastClient>,
}

impl BroadcastServiceBuilder {
    pub fn with_client(mut self, client: BroadcastClient) -> Self {
        self.client = Some(client);
        self
    }

    pub fn build(self) -> Result<BroadcastService, IRacingSDKError> {
        let client = match self.client {
            Some(c) => c,
            None => BroadcastClient::new()?,
        };

        Ok(BroadcastService { client })
    }
}

#[derive(Debug)]
pub struct BroadcastService {
    client: BroadcastClient,
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
            .send_message(message)
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

    fn proto_i32_to_i16(field_name: &'static str, value: i32) -> Result<i16, Status> {
        i16::try_from(value).map_err(|_| {
            Status::invalid_argument(format!(
                "{field_name} must be in the range {}..={}, got {value}",
                i16::MIN,
                i16::MAX
            ))
        })
    }

    fn required_proto_u16(field_name: &'static str, value: Option<u32>) -> Result<u16, Status> {
        match value {
            Some(value) => Self::proto_u32_to_u16(field_name, value),
            None => Err(Status::invalid_argument(format!("Missing `{field_name}`"))),
        }
    }

    fn required_proto_i16(field_name: &'static str, value: Option<i32>) -> Result<i16, Status> {
        match value {
            Some(value) => Self::proto_i32_to_i16(field_name, value),
            None => Err(Status::invalid_argument(format!("Missing `{field_name}`"))),
        }
    }

    fn required_proto_bool(field_name: &'static str, value: Option<bool>) -> Result<bool, Status> {
        value.ok_or_else(|| Status::invalid_argument(format!("Missing `{field_name}`")))
    }

    fn required_proto_string(
        field_name: &'static str,
        value: Option<String>,
    ) -> Result<String, Status> {
        match value {
            Some(value) if !value.is_empty() => Ok(value),
            Some(_) => Err(Status::invalid_argument(format!(
                "`{field_name}` must not be empty"
            ))),
            None => Err(Status::invalid_argument(format!("Missing `{field_name}`"))),
        }
    }

    fn required_chat_macro(value: Option<u32>) -> Result<u16, Status> {
        let macro_number = Self::required_proto_u16("macro", value)?;
        if !(1..=15).contains(&macro_number) {
            return Err(Status::invalid_argument(format!(
                "`macro` must be in the range 1..=15, got {macro_number}"
            )));
        }

        Ok(macro_number)
    }

    fn required_proto_enum<E>(field_name: &'static str, value: Option<i32>) -> Result<E, Status>
    where
        E: TryFrom<i32>,
    {
        let value =
            value.ok_or_else(|| Status::invalid_argument(format!("Missing `{field_name}`")))?;

        if value == 0 {
            return Err(Status::invalid_argument(format!(
                "`{field_name}` must not be UNKNOWN"
            )));
        }

        E::try_from(value).map_err(|_| {
            Status::invalid_argument(format!("Invalid `{field_name}` enum value: {value}"))
        })
    }

    fn required_proto_f32(field_name: &'static str, value: Option<f32>) -> Result<f32, Status> {
        let value =
            value.ok_or_else(|| Status::invalid_argument(format!("Missing `{field_name}`")))?;

        if value.is_finite() {
            Ok(value)
        } else {
            Err(Status::invalid_argument(format!(
                "`{field_name}` must be finite, got {value}"
            )))
        }
    }

    fn proto_f32_to_u16(field_name: &'static str, value: f32) -> Result<u16, Status> {
        if value < 0.0 || value > f32::from(u16::MAX) || value.fract() != 0.0 {
            return Err(Status::invalid_argument(format!(
                "`{field_name}` must be an integer in the range 0..={}, got {value}",
                u16::MAX
            )));
        }

        Ok(value as u16)
    }

    fn replay_position_mode(mode: ReplayPositionMode) -> iracing_sdk::ReplayPositionMode {
        match mode {
            ReplayPositionMode::Begin => iracing_sdk::ReplayPositionMode::Begin,
            ReplayPositionMode::Current => iracing_sdk::ReplayPositionMode::Current,
            ReplayPositionMode::End => iracing_sdk::ReplayPositionMode::End,
            ReplayPositionMode::Unknown => unreachable!("unknown replay position mode is rejected"),
        }
    }

    fn replay_search_mode(mode: ReplaySearchMode) -> iracing_sdk::ReplaySearchMode {
        match mode {
            ReplaySearchMode::ToStart => iracing_sdk::ReplaySearchMode::ToStart,
            ReplaySearchMode::ToEnd => iracing_sdk::ReplaySearchMode::ToEnd,
            ReplaySearchMode::PreviousSession => iracing_sdk::ReplaySearchMode::PrevSession,
            ReplaySearchMode::NextSession => iracing_sdk::ReplaySearchMode::NextSession,
            ReplaySearchMode::PreviousLap => iracing_sdk::ReplaySearchMode::PrevLap,
            ReplaySearchMode::NextLap => iracing_sdk::ReplaySearchMode::NextLap,
            ReplaySearchMode::PreviousFrame => iracing_sdk::ReplaySearchMode::PrevFrame,
            ReplaySearchMode::NextFrame => iracing_sdk::ReplaySearchMode::NextFrame,
            ReplaySearchMode::PreviousIncident => iracing_sdk::ReplaySearchMode::PrevIncident,
            ReplaySearchMode::NextIncident => iracing_sdk::ReplaySearchMode::NextIncident,
            ReplaySearchMode::Unknown => unreachable!("unknown replay search mode is rejected"),
        }
    }

    fn replay_state_mode(mode: ReplayStateMode) -> iracing_sdk::ReplayStateMode {
        match mode {
            ReplayStateMode::EraseTape => iracing_sdk::ReplayStateMode::EraseTape,
            ReplayStateMode::Unknown => unreachable!("unknown replay state mode is rejected"),
        }
    }

    fn chat_command(request: ChatCommandRequest) -> Result<BroadcastCommand, Status> {
        let ChatCommandRequest { mode, r#macro } = request;
        let mode = Self::required_proto_enum::<ChatCommandMode>("mode", mode)?;

        Ok(match mode {
            ChatCommandMode::Macro => {
                let macro_number = Self::required_chat_macro(r#macro)?;
                BroadcastCommand::ChatCommandMacro(macro_number)
            }
            ChatCommandMode::BeginChat => {
                BroadcastCommand::ChatCommand(iracing_sdk::ChatCommandMode::BeginChat)
            }
            ChatCommandMode::Reply => {
                BroadcastCommand::ChatCommand(iracing_sdk::ChatCommandMode::Reply)
            }
            ChatCommandMode::Cancel => {
                BroadcastCommand::ChatCommand(iracing_sdk::ChatCommandMode::Cancel)
            }
            ChatCommandMode::Unknown => unreachable!("unknown chat command mode is rejected"),
        })
    }

    fn pit_command(request: PitCommandRequest) -> Result<PitCommand, Status> {
        let PitCommandRequest { mode, value } = request;
        let mode = Self::required_proto_enum::<PitCommandMode>("mode", mode)?;

        Ok(match mode {
            PitCommandMode::Clear => PitCommand::Clear,
            PitCommandMode::TearOff => PitCommand::Tearoff,
            PitCommandMode::Fuel => {
                let value = Self::required_proto_f32("value", value)?;
                PitCommand::Fuel(Self::proto_f32_to_u16("value", value)?)
            }
            PitCommandMode::LfTire => {
                let value = Self::required_proto_f32("value", value)?;
                PitCommand::LF(Self::proto_f32_to_u16("value", value)?)
            }
            PitCommandMode::RfTire => {
                let value = Self::required_proto_f32("value", value)?;
                PitCommand::RF(Self::proto_f32_to_u16("value", value)?)
            }
            PitCommandMode::LrTire => {
                let value = Self::required_proto_f32("value", value)?;
                PitCommand::LR(Self::proto_f32_to_u16("value", value)?)
            }
            PitCommandMode::RrTire => {
                let value = Self::required_proto_f32("value", value)?;
                PitCommand::RR(Self::proto_f32_to_u16("value", value)?)
            }
            PitCommandMode::ClearTires => PitCommand::ClearTires,
            PitCommandMode::FastRepair => PitCommand::FastRepair,
            PitCommandMode::ClearTearOff => PitCommand::ClearTearoff,
            PitCommandMode::ClearFastRepair => PitCommand::ClearFastRepair,
            PitCommandMode::ClearFuel => PitCommand::ClearFuel,
            PitCommandMode::Unknown => unreachable!("unknown pit command mode is rejected"),
        })
    }

    fn telemetry_command_mode(mode: TelemetryCommandMode) -> iracing_sdk::TelemetryCommandMode {
        match mode {
            TelemetryCommandMode::Stop => iracing_sdk::TelemetryCommandMode::Stop,
            TelemetryCommandMode::Start => iracing_sdk::TelemetryCommandMode::Start,
            TelemetryCommandMode::Restart => iracing_sdk::TelemetryCommandMode::Restart,
            TelemetryCommandMode::Unknown => {
                unreachable!("unknown telemetry command mode is rejected")
            }
        }
    }

    fn video_capture_mode(mode: VideoCaptureMode) -> iracing_sdk::VideoCaptureMode {
        match mode {
            VideoCaptureMode::Screenshot => iracing_sdk::VideoCaptureMode::TriggerScreenShot,
            VideoCaptureMode::Start => iracing_sdk::VideoCaptureMode::StartVideoCapture,
            VideoCaptureMode::Stop => iracing_sdk::VideoCaptureMode::EndVideoCapture,
            VideoCaptureMode::Toggle => iracing_sdk::VideoCaptureMode::ToggleVideoCapture,
            VideoCaptureMode::ShowTimer => iracing_sdk::VideoCaptureMode::ShowVideoTimer,
            VideoCaptureMode::HideTimer => iracing_sdk::VideoCaptureMode::HideVideoTimer,
            VideoCaptureMode::Unknown => unreachable!("unknown video capture mode is rejected"),
        }
    }

    fn empty_pit_command_response() -> PitCommandResponse {
        PitCommandResponse {
            service_flags: 0,
            fuel: 0.0,
            lf_pressure: 0.0,
            rf_pressure: 0.0,
            lr_pressure: 0.0,
            rr_pressure: 0.0,
            tire_compound: 0,
        }
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

        let position = Self::required_proto_u16("position", position)?;
        let group = Self::required_proto_u16("group", group)?;
        let camera = Self::required_proto_u16("camera", camera)?;

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

        let car_number = Self::required_proto_string("car_number", car_number)?;
        let group = Self::required_proto_u16("group", group)?;
        let camera = Self::required_proto_u16("camera", camera)?;

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

        let speed = Self::required_proto_i16("speed", speed)?;
        let is_slow_motion = Self::required_proto_bool("is_slow_motion", is_slow_motion)?;

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

        let mode = Self::required_proto_enum::<ReplayPositionMode>("mode", mode)?;
        let frame = frame.ok_or_else(|| Status::invalid_argument("Missing `frame`"))?;

        // TODO: Get previous replay frame.

        self.send_message(BroadcastCommand::ReplaySetPlayPosition(
            Self::replay_position_mode(mode),
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

        let mode = Self::required_proto_enum::<ReplaySearchMode>("mode", mode)?;

        // TODO: Get previous replay frame/session position.

        self.send_message(BroadcastCommand::ReplaySearch(Self::replay_search_mode(
            mode,
        )))?;

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

        let state = Self::required_proto_enum::<ReplayStateMode>("state", state)?;

        // TODO: Get previous replay state.

        self.send_message(BroadcastCommand::ReplaySetState(Self::replay_state_mode(
            state,
        )))?;

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
                Self::proto_u32_to_u16("car_idx", index).map(BroadcastCommand::ReloadTextures)?
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

        self.send_message(Self::chat_command(request.into_inner())?)?;

        // TODO: Wait for chat command acknowledgement/state if available.

        Ok(Response::new(ChatCommandResponse {}))
    }

    async fn pit_command(
        &self,
        request: Request<PitCommandRequest>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        let command = Self::pit_command(request.into_inner())?;

        // TODO: Get previous pit service state.

        self.send_message(BroadcastCommand::PitCommand(command))?;

        // TODO: Wait for pit service state to change and return selected values.

        Ok(Response::new(Self::empty_pit_command_response()))
    }

    async fn pit_command_stream(
        &self,
        request: Request<tonic::Streaming<PitCommandRequest>>,
    ) -> Result<Response<PitCommandResponse>, Status> {
        let mut stream = request.into_inner();

        // TODO: Get previous pit service state.

        while let Some(request) = stream.message().await? {
            let command = Self::pit_command(request)?;
            self.send_message(BroadcastCommand::PitCommand(command))?;
        }

        // TODO: Wait for pit service state to change and return selected values.

        Ok(Response::new(Self::empty_pit_command_response()))
    }

    async fn telemetry_command(
        &self,
        request: Request<TelemetryCommandRequest>,
    ) -> Result<Response<TelemetryCommandResponse>, Status> {
        let TelemetryCommandRequest { mode } = request.into_inner();

        let mode = Self::required_proto_enum::<TelemetryCommandMode>("mode", mode)?;

        // TODO: Get previous disk telemetry logging state.

        self.send_message(BroadcastCommand::TelemetryCommand(
            Self::telemetry_command_mode(mode),
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

        let mode = Self::required_proto_enum::<ForceFeedbackCommandMode>("mode", mode)?;
        let value = Self::required_proto_f32("value", value)?;

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

        let session_number = Self::required_proto_u16("session_number", session_number)?;
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

        let mode = Self::required_proto_enum::<VideoCaptureMode>("mode", mode)?;

        // TODO: Get previous video capture state if iRacing exposes one.

        self.send_message(BroadcastCommand::VideoCapture(Self::video_capture_mode(
            mode,
        )))?;

        // TODO: Wait for video capture state/acknowledgement if available.

        Ok(Response::new(VideoCaptureResponse {}))
    }
}
