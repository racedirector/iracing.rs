use tonic::{
    Status,
    transport::{Channel, Endpoint},
};

use crate::broadcast::{
    CameraSetStateRequest, CameraSetStateResponse, CameraSwitchNumberRequest,
    CameraSwitchNumberResponse, CameraSwitchPositionRequest, CameraSwitchPositionResponse,
    ChatCommandMode as ProtoChatCommandMode, ChatCommandRequest, ChatCommandResponse,
    ForceFeedbackCommandMode, ForceFeedbackCommandRequest, ForceFeedbackCommandResponse,
    GetAvailableCamerasResponse, PitCommandMode as ProtoPitCommandMode, PitCommandRequest,
    PitCommandResponse, ReloadTexturesRequest, ReloadTexturesResponse,
    ReplayPositionMode as ProtoReplayPositionMode, ReplaySearchMode as ProtoReplaySearchMode,
    ReplaySearchRequest, ReplaySearchResponse, ReplaySearchSessionTimeRequest,
    ReplaySearchSessionTimeResponse, ReplaySetPlayPositionRequest, ReplaySetPlayPositionResponse,
    ReplaySetPlaySpeedRequest, ReplaySetPlaySpeedResponse, ReplaySetStateRequest,
    ReplaySetStateResponse, ReplayStateMode as ProtoReplayStateMode,
    TelemetryCommandMode as ProtoTelemetryCommandMode, TelemetryCommandRequest,
    TelemetryCommandResponse, VideoCaptureMode as ProtoVideoCaptureMode, VideoCaptureRequest,
    VideoCaptureResponse, broadcast_client::BroadcastClient as TonicBroadcastClient,
};

/// Result type used by the ergonomic broadcast gRPC client.
pub type BroadcastGrpcResult<T> = std::result::Result<T, Status>;

/// Ergonomic client for the iRacing broadcast gRPC service.
#[derive(Debug, Clone)]
pub struct BroadcastGrpcClient {
    inner: TonicBroadcastClient<Channel>,
}

impl BroadcastGrpcClient {
    /// Connect to an iRacing broadcast gRPC server.
    ///
    /// `dst` accepts the same endpoint forms as tonic, such as
    /// `http://[::1]:50051`.
    pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
    where
        D: TryInto<Endpoint>,
        D::Error: Into<tonic::codegen::StdError>,
    {
        let inner = TonicBroadcastClient::connect(dst).await?;
        Ok(Self { inner })
    }

    #[must_use]
    pub fn from_channel(channel: Channel) -> Self {
        Self {
            inner: TonicBroadcastClient::new(channel),
        }
    }

    #[must_use]
    pub fn from_inner(inner: TonicBroadcastClient<Channel>) -> Self {
        Self { inner }
    }

    #[must_use]
    pub fn raw_client(&self) -> &TonicBroadcastClient<Channel> {
        &self.inner
    }

    pub fn raw_client_mut(&mut self) -> &mut TonicBroadcastClient<Channel> {
        &mut self.inner
    }

    #[must_use]
    pub fn into_inner(self) -> TonicBroadcastClient<Channel> {
        self.inner
    }

    pub async fn get_available_cameras(
        &mut self,
    ) -> BroadcastGrpcResult<GetAvailableCamerasResponse> {
        self.inner
            .get_available_cameras(())
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn camera_switch_position(
        &mut self,
        position: u16,
        group: u16,
        camera: u16,
    ) -> BroadcastGrpcResult<CameraSwitchPositionResponse> {
        self.inner
            .camera_switch_position(CameraSwitchPositionRequest {
                position: Some(u32::from(position)),
                group: Some(u32::from(group)),
                camera: Some(u32::from(camera)),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn camera_switch_number(
        &mut self,
        car_number: impl Into<String>,
        group: u16,
        camera: u16,
    ) -> BroadcastGrpcResult<CameraSwitchNumberResponse> {
        self.inner
            .camera_switch_number(CameraSwitchNumberRequest {
                car_number: Some(car_number.into()),
                group: Some(u32::from(group)),
                camera: Some(u32::from(camera)),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn camera_set_state(
        &mut self,
        state: iracing_sdk::CameraState,
    ) -> BroadcastGrpcResult<CameraSetStateResponse> {
        self.inner
            .camera_set_state(CameraSetStateRequest {
                state: Some(state.bits()),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn replay_set_play_speed(
        &mut self,
        speed: u16,
        is_slow_motion: bool,
    ) -> BroadcastGrpcResult<ReplaySetPlaySpeedResponse> {
        if speed > i16::MAX as u16 {
            return Err(Status::invalid_argument(format!(
                "speed must be in the range 0..={}, got {speed}",
                i16::MAX
            )));
        }

        self.inner
            .replay_set_play_speed(ReplaySetPlaySpeedRequest {
                speed: Some(u32::from(speed)),
                is_slow_motion: Some(is_slow_motion),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn replay_set_play_position(
        &mut self,
        mode: iracing_sdk::ReplayPositionMode,
        frame: u16,
    ) -> BroadcastGrpcResult<ReplaySetPlayPositionResponse> {
        self.inner
            .replay_set_play_position(ReplaySetPlayPositionRequest {
                mode: Some(replay_position_mode(mode)? as i32),
                frame: Some(u32::from(frame)),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn replay_search(
        &mut self,
        mode: iracing_sdk::ReplaySearchMode,
    ) -> BroadcastGrpcResult<ReplaySearchResponse> {
        self.inner
            .replay_search(ReplaySearchRequest {
                mode: Some(replay_search_mode(mode)? as i32),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn replay_set_state(
        &mut self,
        state: iracing_sdk::ReplayStateMode,
    ) -> BroadcastGrpcResult<ReplaySetStateResponse> {
        self.inner
            .replay_set_state(ReplaySetStateRequest {
                state: Some(replay_state_mode(state)? as i32),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn reload_all_textures(&mut self) -> BroadcastGrpcResult<ReloadTexturesResponse> {
        self.reload_textures(None).await
    }

    pub async fn reload_car_textures(
        &mut self,
        car_idx: u16,
    ) -> BroadcastGrpcResult<ReloadTexturesResponse> {
        self.reload_textures(Some(car_idx)).await
    }

    pub async fn reload_textures(
        &mut self,
        car_idx: Option<u16>,
    ) -> BroadcastGrpcResult<ReloadTexturesResponse> {
        self.inner
            .reload_textures(ReloadTexturesRequest {
                car_idx: car_idx.map(u32::from),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn chat_macro(
        &mut self,
        macro_number: u16,
    ) -> BroadcastGrpcResult<ChatCommandResponse> {
        self.inner
            .chat_command(ChatCommandRequest {
                mode: Some(ProtoChatCommandMode::Macro as i32),
                r#macro: Some(u32::from(macro_number)),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn chat_command(
        &mut self,
        mode: iracing_sdk::ChatCommandMode,
    ) -> BroadcastGrpcResult<ChatCommandResponse> {
        self.inner
            .chat_command(ChatCommandRequest {
                mode: Some(chat_command_mode(mode)? as i32),
                r#macro: None,
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn pit_command(
        &mut self,
        command: iracing_sdk::PitCommand,
    ) -> BroadcastGrpcResult<PitCommandResponse> {
        self.inner
            .pit_command(pit_command_request(command))
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn pit_command_stream(
        &mut self,
        commands: impl IntoIterator<Item = iracing_sdk::PitCommand>,
    ) -> BroadcastGrpcResult<PitCommandResponse> {
        let requests = commands
            .into_iter()
            .map(pit_command_request)
            .collect::<Vec<_>>();
        let stream = tonic::codegen::tokio_stream::iter(requests);

        self.inner
            .pit_command_stream(stream)
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn telemetry_command(
        &mut self,
        mode: iracing_sdk::TelemetryCommandMode,
    ) -> BroadcastGrpcResult<TelemetryCommandResponse> {
        self.inner
            .telemetry_command(TelemetryCommandRequest {
                mode: Some(telemetry_command_mode(mode)? as i32),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn force_feedback_max_force(
        &mut self,
        max_force: f32,
    ) -> BroadcastGrpcResult<ForceFeedbackCommandResponse> {
        if !max_force.is_finite() {
            return Err(Status::invalid_argument(format!(
                "max_force must be finite, got {max_force}"
            )));
        }

        self.inner
            .force_feedback_command(ForceFeedbackCommandRequest {
                mode: Some(ForceFeedbackCommandMode::MaxForce as i32),
                value: Some(max_force),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn replay_search_session_time(
        &mut self,
        session_number: u16,
        session_time_ms: u32,
    ) -> BroadcastGrpcResult<ReplaySearchSessionTimeResponse> {
        let session_time_ms = u32_as_proto_float("session_time_ms", session_time_ms)?;

        self.inner
            .replay_search_session_time(ReplaySearchSessionTimeRequest {
                session_number: Some(u32::from(session_number)),
                session_time_ms: Some(session_time_ms),
            })
            .await
            .map(tonic::Response::into_inner)
    }

    pub async fn video_capture(
        &mut self,
        mode: iracing_sdk::VideoCaptureMode,
    ) -> BroadcastGrpcResult<VideoCaptureResponse> {
        self.inner
            .video_capture(VideoCaptureRequest {
                mode: Some(video_capture_mode(mode)? as i32),
            })
            .await
            .map(tonic::Response::into_inner)
    }
}

fn chat_command_mode(
    mode: iracing_sdk::ChatCommandMode,
) -> BroadcastGrpcResult<ProtoChatCommandMode> {
    match mode {
        iracing_sdk::ChatCommandMode::BeginChat => Ok(ProtoChatCommandMode::BeginChat),
        iracing_sdk::ChatCommandMode::Reply => Ok(ProtoChatCommandMode::Reply),
        iracing_sdk::ChatCommandMode::Cancel => Ok(ProtoChatCommandMode::Cancel),
        iracing_sdk::ChatCommandMode::Macro => Err(Status::invalid_argument(
            "`mode` macro requires `chat_macro(macro_number)`",
        )),
        iracing_sdk::ChatCommandMode::Unknown(value) => {
            Err(unknown_sdk_enum("ChatCommandMode", value))
        }
    }
}

fn pit_command_request(command: iracing_sdk::PitCommand) -> PitCommandRequest {
    let (mode, value) = match command {
        iracing_sdk::PitCommand::Clear => (ProtoPitCommandMode::Clear, None),
        iracing_sdk::PitCommand::Tearoff => (ProtoPitCommandMode::TearOff, None),
        iracing_sdk::PitCommand::Fuel(gallons) => {
            (ProtoPitCommandMode::Fuel, Some(f32::from(gallons)))
        }
        iracing_sdk::PitCommand::LF(pressure) => {
            (ProtoPitCommandMode::LfTire, Some(f32::from(pressure)))
        }
        iracing_sdk::PitCommand::RF(pressure) => {
            (ProtoPitCommandMode::RfTire, Some(f32::from(pressure)))
        }
        iracing_sdk::PitCommand::LR(pressure) => {
            (ProtoPitCommandMode::LrTire, Some(f32::from(pressure)))
        }
        iracing_sdk::PitCommand::RR(pressure) => {
            (ProtoPitCommandMode::RrTire, Some(f32::from(pressure)))
        }
        iracing_sdk::PitCommand::ClearTires => (ProtoPitCommandMode::ClearTires, None),
        iracing_sdk::PitCommand::FastRepair => (ProtoPitCommandMode::FastRepair, None),
        iracing_sdk::PitCommand::ClearTearoff => (ProtoPitCommandMode::ClearTearOff, None),
        iracing_sdk::PitCommand::ClearFastRepair => (ProtoPitCommandMode::ClearFastRepair, None),
        iracing_sdk::PitCommand::ClearFuel => (ProtoPitCommandMode::ClearFuel, None),
    };

    PitCommandRequest {
        mode: Some(mode as i32),
        value,
    }
}

fn telemetry_command_mode(
    mode: iracing_sdk::TelemetryCommandMode,
) -> BroadcastGrpcResult<ProtoTelemetryCommandMode> {
    match mode {
        iracing_sdk::TelemetryCommandMode::Stop => Ok(ProtoTelemetryCommandMode::Stop),
        iracing_sdk::TelemetryCommandMode::Start => Ok(ProtoTelemetryCommandMode::Start),
        iracing_sdk::TelemetryCommandMode::Restart => Ok(ProtoTelemetryCommandMode::Restart),
        iracing_sdk::TelemetryCommandMode::Unknown(value) => {
            Err(unknown_sdk_enum("TelemetryCommandMode", value))
        }
    }
}

fn replay_position_mode(
    mode: iracing_sdk::ReplayPositionMode,
) -> BroadcastGrpcResult<ProtoReplayPositionMode> {
    match mode {
        iracing_sdk::ReplayPositionMode::Begin => Ok(ProtoReplayPositionMode::Begin),
        iracing_sdk::ReplayPositionMode::Current => Ok(ProtoReplayPositionMode::Current),
        iracing_sdk::ReplayPositionMode::End => Ok(ProtoReplayPositionMode::End),
        iracing_sdk::ReplayPositionMode::Last => {
            Err(unsupported_sdk_enum("ReplayPositionMode::Last"))
        }
        iracing_sdk::ReplayPositionMode::Unknown(value) => {
            Err(unknown_sdk_enum("ReplayPositionMode", value))
        }
    }
}

fn replay_search_mode(
    mode: iracing_sdk::ReplaySearchMode,
) -> BroadcastGrpcResult<ProtoReplaySearchMode> {
    match mode {
        iracing_sdk::ReplaySearchMode::ToStart => Ok(ProtoReplaySearchMode::ToStart),
        iracing_sdk::ReplaySearchMode::ToEnd => Ok(ProtoReplaySearchMode::ToEnd),
        iracing_sdk::ReplaySearchMode::PrevSession => Ok(ProtoReplaySearchMode::PreviousSession),
        iracing_sdk::ReplaySearchMode::NextSession => Ok(ProtoReplaySearchMode::NextSession),
        iracing_sdk::ReplaySearchMode::PrevLap => Ok(ProtoReplaySearchMode::PreviousLap),
        iracing_sdk::ReplaySearchMode::NextLap => Ok(ProtoReplaySearchMode::NextLap),
        iracing_sdk::ReplaySearchMode::PrevFrame => Ok(ProtoReplaySearchMode::PreviousFrame),
        iracing_sdk::ReplaySearchMode::NextFrame => Ok(ProtoReplaySearchMode::NextFrame),
        iracing_sdk::ReplaySearchMode::PrevIncident => Ok(ProtoReplaySearchMode::PreviousIncident),
        iracing_sdk::ReplaySearchMode::NextIncident => Ok(ProtoReplaySearchMode::NextIncident),
        iracing_sdk::ReplaySearchMode::Last => Err(unsupported_sdk_enum("ReplaySearchMode::Last")),
        iracing_sdk::ReplaySearchMode::Unknown(value) => {
            Err(unknown_sdk_enum("ReplaySearchMode", value))
        }
    }
}

fn replay_state_mode(
    mode: iracing_sdk::ReplayStateMode,
) -> BroadcastGrpcResult<ProtoReplayStateMode> {
    match mode {
        iracing_sdk::ReplayStateMode::EraseTape => Ok(ProtoReplayStateMode::EraseTape),
        iracing_sdk::ReplayStateMode::Last => Err(unsupported_sdk_enum("ReplayStateMode::Last")),
        iracing_sdk::ReplayStateMode::Unknown(value) => {
            Err(unknown_sdk_enum("ReplayStateMode", value))
        }
    }
}

fn video_capture_mode(
    mode: iracing_sdk::VideoCaptureMode,
) -> BroadcastGrpcResult<ProtoVideoCaptureMode> {
    match mode {
        iracing_sdk::VideoCaptureMode::TriggerScreenShot => Ok(ProtoVideoCaptureMode::Screenshot),
        iracing_sdk::VideoCaptureMode::StartVideoCapture => Ok(ProtoVideoCaptureMode::Start),
        iracing_sdk::VideoCaptureMode::EndVideoCapture => Ok(ProtoVideoCaptureMode::Stop),
        iracing_sdk::VideoCaptureMode::ToggleVideoCapture => Ok(ProtoVideoCaptureMode::Toggle),
        iracing_sdk::VideoCaptureMode::ShowVideoTimer => Ok(ProtoVideoCaptureMode::ShowTimer),
        iracing_sdk::VideoCaptureMode::HideVideoTimer => Ok(ProtoVideoCaptureMode::HideTimer),
        iracing_sdk::VideoCaptureMode::Unknown(value) => {
            Err(unknown_sdk_enum("VideoCaptureMode", value))
        }
    }
}

fn unknown_sdk_enum(enum_name: &'static str, value: i32) -> Status {
    Status::invalid_argument(format!("Unknown {enum_name} value: {value}"))
}

fn unsupported_sdk_enum(variant: &'static str) -> Status {
    Status::invalid_argument(format!("Unsupported broadcast enum variant: {variant}"))
}

fn u32_as_proto_float(field_name: &'static str, value: u32) -> BroadcastGrpcResult<f32> {
    let encoded = value as f32;

    if encoded as u32 == value {
        Ok(encoded)
    } else {
        Err(Status::invalid_argument(format!(
            "`{field_name}` cannot be represented by the broadcast protobuf float without changing value: {value}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_state_uses_sdk_bitfield_bits() {
        let state =
            iracing_sdk::CameraState::CAM_TOOL_ACTIVE.union(iracing_sdk::CameraState::UI_HIDDEN);
        assert_eq!(state.bits(), 0x000c);
    }

    #[test]
    fn pit_command_builds_request_from_sdk_type() {
        assert_eq!(
            pit_command_request(iracing_sdk::PitCommand::Fuel(8)),
            PitCommandRequest {
                mode: Some(ProtoPitCommandMode::Fuel as i32),
                value: Some(8.0)
            }
        );
    }

    #[test]
    fn rejects_unsupported_sdk_enum_variant() {
        assert!(replay_search_mode(iracing_sdk::ReplaySearchMode::Last).is_err());
    }

    #[test]
    fn rejects_float_rounding_for_session_time() {
        assert!(u32_as_proto_float("session_time_ms", 16_777_217).is_err());
    }
}
