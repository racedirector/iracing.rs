use iracing_broadcast_grpc_service::{
    CameraDetail as ProtoCameraDetail, CameraGroup as ProtoCameraGroup,
    CameraSetStateRequest as ProtoCameraSetStateRequest,
    CameraSetStateResponse as ProtoCameraSetStateResponse,
    CameraSwitchNumberRequest as ProtoCameraSwitchNumberRequest,
    CameraSwitchNumberResponse as ProtoCameraSwitchNumberResponse,
    CameraSwitchPositionRequest as ProtoCameraSwitchPositionRequest,
    CameraSwitchPositionResponse as ProtoCameraSwitchPositionResponse,
    ChatCommandMode as ProtoChatCommandMode, ChatCommandRequest as ProtoChatCommandRequest,
    ForceFeedbackCommandMode as ProtoForceFeedbackCommandMode,
    ForceFeedbackCommandRequest as ProtoForceFeedbackCommandRequest,
    ForceFeedbackCommandResponse as ProtoForceFeedbackCommandResponse,
    GetAvailableCamerasResponse as ProtoGetAvailableCamerasResponse,
    PitCommandMode as ProtoPitCommandMode, PitCommandRequest as ProtoPitCommandRequest,
    PitCommandResponse as ProtoPitCommandResponse, RawBroadcastClient,
    ReloadTexturesRequest as ProtoReloadTexturesRequest,
    ReplayPositionMode as ProtoReplayPositionMode, ReplaySearchMode as ProtoReplaySearchMode,
    ReplaySearchRequest as ProtoReplaySearchRequest,
    ReplaySearchResponse as ProtoReplaySearchResponse,
    ReplaySearchSessionTimeRequest as ProtoReplaySearchSessionTimeRequest,
    ReplaySetPlayPositionRequest as ProtoReplaySetPlayPositionRequest,
    ReplaySetPlayPositionResponse as ProtoReplaySetPlayPositionResponse,
    ReplaySetPlaySpeedRequest as ProtoReplaySetPlaySpeedRequest,
    ReplaySetPlaySpeedResponse as ProtoReplaySetPlaySpeedResponse,
    ReplaySetStateRequest as ProtoReplaySetStateRequest, ReplayStateMode as ProtoReplayStateMode,
    TelemetryCommandMode as ProtoTelemetryCommandMode,
    TelemetryCommandRequest as ProtoTelemetryCommandRequest,
    TelemetryCommandResponse as ProtoTelemetryCommandResponse,
    VideoCaptureMode as ProtoVideoCaptureMode, VideoCaptureRequest as ProtoVideoCaptureRequest,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::server::ServerManager;

#[derive(Debug, Deserialize)]
#[serde(tag = "message", content = "values")]
pub enum BroadcastClientRequest {
    CameraSetStateRequest(CameraSetStateRequest),
    CameraSwitchNumberRequest(CameraSwitchNumberRequest),
    CameraSwitchPositionRequest(CameraSwitchPositionRequest),
    ChatCommandRequest(ChatCommandRequest),
    ForceFeedbackCommandRequest(ForceFeedbackCommandRequest),
    GetAvailableCamerasRequest(EmptyRequest),
    PitCommandRequest(PitCommandRequest),
    ReloadTexturesRequest(ReloadTexturesRequest),
    ReplaySearchRequest(ReplaySearchRequest),
    ReplaySearchSessionTimeRequest(ReplaySearchSessionTimeRequest),
    ReplaySetPlayPositionRequest(ReplaySetPlayPositionRequest),
    ReplaySetPlaySpeedRequest(ReplaySetPlaySpeedRequest),
    ReplaySetStateRequest(ReplaySetStateRequest),
    TelemetryCommandRequest(TelemetryCommandRequest),
    VideoCaptureRequest(VideoCaptureRequest),
}

#[derive(Debug, Deserialize)]
pub struct EmptyRequest {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraSwitchPositionRequest {
    position: u32,
    group: u32,
    camera: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraSwitchNumberRequest {
    car_number: String,
    group: u32,
    camera: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraSetStateRequest {
    state: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySetPlaySpeedRequest {
    speed: i32,
    is_slow_motion: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySetPlayPositionRequest {
    mode: ReplayPositionMode,
    frame: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySearchRequest {
    mode: ReplaySearchMode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySetStateRequest {
    state: ReplayStateMode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReloadTexturesRequest {
    car_idx: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCommandRequest {
    mode: ChatCommandMode,
    r#macro: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PitCommandRequest {
    mode: PitCommandMode,
    value: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryCommandRequest {
    mode: TelemetryCommandMode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForceFeedbackCommandRequest {
    mode: ForceFeedbackCommandMode,
    value: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySearchSessionTimeRequest {
    session_number: u32,
    session_time_ms: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoCaptureRequest {
    mode: VideoCaptureMode,
}

#[derive(Debug, Deserialize)]
pub enum ChatCommandMode {
    #[serde(rename = "CHAT_COMMAND_MODE_MACRO")]
    Macro,
    #[serde(rename = "CHAT_COMMAND_MODE_BEGIN_CHAT")]
    BeginChat,
    #[serde(rename = "CHAT_COMMAND_MODE_REPLY")]
    Reply,
    #[serde(rename = "CHAT_COMMAND_MODE_CANCEL")]
    Cancel,
}

#[derive(Debug, Deserialize)]
pub enum ForceFeedbackCommandMode {
    #[serde(rename = "FORCE_FEEDBACK_COMMAND_MODE_MAX_FORCE")]
    MaxForce,
}

#[derive(Debug, Deserialize)]
pub enum PitCommandMode {
    #[serde(rename = "PIT_COMMAND_MODE_CLEAR")]
    Clear,
    #[serde(rename = "PIT_COMMAND_MODE_TEAR_OFF")]
    TearOff,
    #[serde(rename = "PIT_COMMAND_MODE_FUEL")]
    Fuel,
    #[serde(rename = "PIT_COMMAND_MODE_LF_TIRE")]
    LfTire,
    #[serde(rename = "PIT_COMMAND_MODE_RF_TIRE")]
    RfTire,
    #[serde(rename = "PIT_COMMAND_MODE_LR_TIRE")]
    LrTire,
    #[serde(rename = "PIT_COMMAND_MODE_RR_TIRE")]
    RrTire,
    #[serde(rename = "PIT_COMMAND_MODE_CLEAR_TIRES")]
    ClearTires,
    #[serde(rename = "PIT_COMMAND_MODE_FAST_REPAIR")]
    FastRepair,
    #[serde(rename = "PIT_COMMAND_MODE_CLEAR_TEAR_OFF")]
    ClearTearOff,
    #[serde(rename = "PIT_COMMAND_MODE_CLEAR_FAST_REPAIR")]
    ClearFastRepair,
    #[serde(rename = "PIT_COMMAND_MODE_CLEAR_FUEL")]
    ClearFuel,
}

#[derive(Debug, Deserialize)]
pub enum ReplayPositionMode {
    #[serde(rename = "REPLAY_POSITION_MODE_BEGIN")]
    Begin,
    #[serde(rename = "REPLAY_POSITION_MODE_CURRENT")]
    Current,
    #[serde(rename = "REPLAY_POSITION_MODE_END")]
    End,
}

#[derive(Debug, Deserialize)]
pub enum ReplaySearchMode {
    #[serde(rename = "REPLAY_SEARCH_MODE_TO_START")]
    ToStart,
    #[serde(rename = "REPLAY_SEARCH_MODE_TO_END")]
    ToEnd,
    #[serde(rename = "REPLAY_SEARCH_MODE_PREVIOUS_SESSION")]
    PreviousSession,
    #[serde(rename = "REPLAY_SEARCH_MODE_NEXT_SESSION")]
    NextSession,
    #[serde(rename = "REPLAY_SEARCH_MODE_PREVIOUS_LAP")]
    PreviousLap,
    #[serde(rename = "REPLAY_SEARCH_MODE_NEXT_LAP")]
    NextLap,
    #[serde(rename = "REPLAY_SEARCH_MODE_PREVIOUS_FRAME")]
    PreviousFrame,
    #[serde(rename = "REPLAY_SEARCH_MODE_NEXT_FRAME")]
    NextFrame,
    #[serde(rename = "REPLAY_SEARCH_MODE_PREVIOUS_INCIDENT")]
    PreviousIncident,
    #[serde(rename = "REPLAY_SEARCH_MODE_NEXT_INCIDENT")]
    NextIncident,
}

#[derive(Debug, Deserialize)]
pub enum ReplayStateMode {
    #[serde(rename = "REPLAY_STATE_MODE_ERASE_TAPE")]
    EraseTape,
}

#[derive(Debug, Deserialize)]
pub enum TelemetryCommandMode {
    #[serde(rename = "TELEMETRY_COMMAND_MODE_STOP")]
    Stop,
    #[serde(rename = "TELEMETRY_COMMAND_MODE_START")]
    Start,
    #[serde(rename = "TELEMETRY_COMMAND_MODE_RESTART")]
    Restart,
}

#[derive(Debug, Deserialize)]
pub enum VideoCaptureMode {
    #[serde(rename = "VIDEO_CAPTURE_MODE_SCREENSHOT")]
    Screenshot,
    #[serde(rename = "VIDEO_CAPTURE_MODE_START")]
    Start,
    #[serde(rename = "VIDEO_CAPTURE_MODE_STOP")]
    Stop,
    #[serde(rename = "VIDEO_CAPTURE_MODE_TOGGLE")]
    Toggle,
    #[serde(rename = "VIDEO_CAPTURE_MODE_SHOW_TIMER")]
    ShowTimer,
    #[serde(rename = "VIDEO_CAPTURE_MODE_HIDE_TIMER")]
    HideTimer,
}

#[derive(Debug, Serialize)]
#[serde(tag = "message", content = "values")]
pub enum BroadcastClientResponse {
    CameraSetStateResponse(CameraSetStateResponse),
    CameraSwitchNumberResponse(CameraSwitchNumberResponse),
    CameraSwitchPositionResponse(CameraSwitchPositionResponse),
    ChatCommandResponse(EmptyResponse),
    ForceFeedbackCommandResponse(ForceFeedbackCommandResponse),
    GetAvailableCamerasResponse(GetAvailableCamerasResponse),
    PitCommandResponse(PitCommandResponse),
    ReloadTexturesResponse(EmptyResponse),
    ReplaySearchResponse(ReplaySearchResponse),
    ReplaySearchSessionTimeResponse(EmptyResponse),
    ReplaySetPlayPositionResponse(ReplaySetPlayPositionResponse),
    ReplaySetPlaySpeedResponse(ReplaySetPlaySpeedResponse),
    ReplaySetStateResponse(EmptyResponse),
    TelemetryCommandResponse(TelemetryCommandResponse),
    VideoCaptureResponse(EmptyResponse),
}

#[derive(Debug, Serialize)]
pub struct EmptyResponse {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraSwitchPositionResponse {
    car_index: u32,
    group: u32,
    camera: u32,
}

pub type CameraSwitchNumberResponse = CameraSwitchPositionResponse;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraSetStateResponse {
    state: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySetPlaySpeedResponse {
    speed: i32,
    is_slow_motion: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySetPlayPositionResponse {
    frame: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySearchResponse {
    frame: u32,
    session_number: u32,
    session_time: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PitCommandResponse {
    service_flags: u32,
    fuel: f32,
    lf_pressure: f32,
    rf_pressure: f32,
    lr_pressure: f32,
    rr_pressure: f32,
    tire_compound: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryCommandResponse {
    is_disk_logging_enabled: bool,
    is_disk_logging_active: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForceFeedbackCommandResponse {
    max_force: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraDetail {
    number: Option<u32>,
    name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraGroup {
    number: u32,
    name: String,
    cameras: Vec<CameraDetail>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAvailableCamerasResponse {
    camera_groups: Vec<CameraGroup>,
    car_index: u32,
    group: u32,
    camera: u32,
}

#[tauri::command]
pub async fn send_broadcast_client_request(
    manager: State<'_, ServerManager>,
    request: BroadcastClientRequest,
) -> Result<BroadcastClientResponse, String> {
    let endpoint = manager.grpc_endpoint()?;
    send_request_to_endpoint(endpoint, request).await
}

async fn send_request_to_endpoint(
    endpoint: String,
    request: BroadcastClientRequest,
) -> Result<BroadcastClientResponse, String> {
    let mut client = RawBroadcastClient::connect(endpoint.clone())
        .await
        .map_err(|error| format!("gRPC client failed to connect to {endpoint}: {error}"))?;

    match request {
        BroadcastClientRequest::CameraSetStateRequest(request) => client
            .camera_set_state(ProtoCameraSetStateRequest {
                state: Some(request.state),
            })
            .await
            .map(tonic::Response::into_inner)
            .map(CameraSetStateResponse::from)
            .map(BroadcastClientResponse::CameraSetStateResponse)
            .map_err(|error| format!("CameraSetStateRequest failed: {error}")),
        BroadcastClientRequest::CameraSwitchNumberRequest(request) => client
            .camera_switch_number(ProtoCameraSwitchNumberRequest {
                car_number: Some(request.car_number),
                group: Some(request.group),
                camera: Some(request.camera),
            })
            .await
            .map(tonic::Response::into_inner)
            .map(CameraSwitchNumberResponse::from)
            .map(BroadcastClientResponse::CameraSwitchNumberResponse)
            .map_err(|error| format!("CameraSwitchNumberRequest failed: {error}")),
        BroadcastClientRequest::CameraSwitchPositionRequest(request) => client
            .camera_switch_position(ProtoCameraSwitchPositionRequest {
                position: Some(request.position),
                group: Some(request.group),
                camera: Some(request.camera),
            })
            .await
            .map(tonic::Response::into_inner)
            .map(CameraSwitchPositionResponse::from)
            .map(BroadcastClientResponse::CameraSwitchPositionResponse)
            .map_err(|error| format!("CameraSwitchPositionRequest failed: {error}")),
        BroadcastClientRequest::ChatCommandRequest(request) => client
            .chat_command(ProtoChatCommandRequest {
                mode: Some(ProtoChatCommandMode::from(request.mode) as i32),
                r#macro: request.r#macro,
            })
            .await
            .map(|_| BroadcastClientResponse::ChatCommandResponse(EmptyResponse {}))
            .map_err(|error| format!("ChatCommandRequest failed: {error}")),
        BroadcastClientRequest::ForceFeedbackCommandRequest(request) => client
            .force_feedback_command(ProtoForceFeedbackCommandRequest {
                mode: Some(ProtoForceFeedbackCommandMode::from(request.mode) as i32),
                value: Some(request.value),
            })
            .await
            .map(tonic::Response::into_inner)
            .map(ForceFeedbackCommandResponse::from)
            .map(BroadcastClientResponse::ForceFeedbackCommandResponse)
            .map_err(|error| format!("ForceFeedbackCommandRequest failed: {error}")),
        BroadcastClientRequest::GetAvailableCamerasRequest(_) => client
            .get_available_cameras(())
            .await
            .map(tonic::Response::into_inner)
            .map(GetAvailableCamerasResponse::from)
            .map(BroadcastClientResponse::GetAvailableCamerasResponse)
            .map_err(|error| format!("GetAvailableCamerasRequest failed: {error}")),
        BroadcastClientRequest::PitCommandRequest(request) => client
            .pit_command(ProtoPitCommandRequest {
                mode: Some(ProtoPitCommandMode::from(request.mode) as i32),
                value: request.value,
            })
            .await
            .map(tonic::Response::into_inner)
            .map(PitCommandResponse::from)
            .map(BroadcastClientResponse::PitCommandResponse)
            .map_err(|error| format!("PitCommandRequest failed: {error}")),
        BroadcastClientRequest::ReloadTexturesRequest(request) => client
            .reload_textures(ProtoReloadTexturesRequest {
                car_idx: request.car_idx,
            })
            .await
            .map(|_| BroadcastClientResponse::ReloadTexturesResponse(EmptyResponse {}))
            .map_err(|error| format!("ReloadTexturesRequest failed: {error}")),
        BroadcastClientRequest::ReplaySearchRequest(request) => client
            .replay_search(ProtoReplaySearchRequest {
                mode: Some(ProtoReplaySearchMode::from(request.mode) as i32),
            })
            .await
            .map(tonic::Response::into_inner)
            .map(ReplaySearchResponse::from)
            .map(BroadcastClientResponse::ReplaySearchResponse)
            .map_err(|error| format!("ReplaySearchRequest failed: {error}")),
        BroadcastClientRequest::ReplaySearchSessionTimeRequest(request) => client
            .replay_search_session_time(ProtoReplaySearchSessionTimeRequest {
                session_number: Some(request.session_number),
                session_time_ms: Some(request.session_time_ms),
            })
            .await
            .map(|_| BroadcastClientResponse::ReplaySearchSessionTimeResponse(EmptyResponse {}))
            .map_err(|error| format!("ReplaySearchSessionTimeRequest failed: {error}")),
        BroadcastClientRequest::ReplaySetPlayPositionRequest(request) => client
            .replay_set_play_position(ProtoReplaySetPlayPositionRequest {
                mode: Some(ProtoReplayPositionMode::from(request.mode) as i32),
                frame: Some(request.frame),
            })
            .await
            .map(tonic::Response::into_inner)
            .map(ReplaySetPlayPositionResponse::from)
            .map(BroadcastClientResponse::ReplaySetPlayPositionResponse)
            .map_err(|error| format!("ReplaySetPlayPositionRequest failed: {error}")),
        BroadcastClientRequest::ReplaySetPlaySpeedRequest(request) => client
            .replay_set_play_speed(ProtoReplaySetPlaySpeedRequest {
                speed: Some(request.speed),
                is_slow_motion: Some(request.is_slow_motion),
            })
            .await
            .map(tonic::Response::into_inner)
            .map(ReplaySetPlaySpeedResponse::from)
            .map(BroadcastClientResponse::ReplaySetPlaySpeedResponse)
            .map_err(|error| format!("ReplaySetPlaySpeedRequest failed: {error}")),
        BroadcastClientRequest::ReplaySetStateRequest(request) => client
            .replay_set_state(ProtoReplaySetStateRequest {
                state: Some(ProtoReplayStateMode::from(request.state) as i32),
            })
            .await
            .map(|_| BroadcastClientResponse::ReplaySetStateResponse(EmptyResponse {}))
            .map_err(|error| format!("ReplaySetStateRequest failed: {error}")),
        BroadcastClientRequest::TelemetryCommandRequest(request) => client
            .telemetry_command(ProtoTelemetryCommandRequest {
                mode: Some(ProtoTelemetryCommandMode::from(request.mode) as i32),
            })
            .await
            .map(tonic::Response::into_inner)
            .map(TelemetryCommandResponse::from)
            .map(BroadcastClientResponse::TelemetryCommandResponse)
            .map_err(|error| format!("TelemetryCommandRequest failed: {error}")),
        BroadcastClientRequest::VideoCaptureRequest(request) => client
            .video_capture(ProtoVideoCaptureRequest {
                mode: Some(ProtoVideoCaptureMode::from(request.mode) as i32),
            })
            .await
            .map(|_| BroadcastClientResponse::VideoCaptureResponse(EmptyResponse {}))
            .map_err(|error| format!("VideoCaptureRequest failed: {error}")),
    }
}

impl From<ChatCommandMode> for ProtoChatCommandMode {
    fn from(mode: ChatCommandMode) -> Self {
        match mode {
            ChatCommandMode::Macro => Self::Macro,
            ChatCommandMode::BeginChat => Self::BeginChat,
            ChatCommandMode::Reply => Self::Reply,
            ChatCommandMode::Cancel => Self::Cancel,
        }
    }
}

impl From<ForceFeedbackCommandMode> for ProtoForceFeedbackCommandMode {
    fn from(mode: ForceFeedbackCommandMode) -> Self {
        match mode {
            ForceFeedbackCommandMode::MaxForce => Self::MaxForce,
        }
    }
}

impl From<PitCommandMode> for ProtoPitCommandMode {
    fn from(mode: PitCommandMode) -> Self {
        match mode {
            PitCommandMode::Clear => Self::Clear,
            PitCommandMode::TearOff => Self::TearOff,
            PitCommandMode::Fuel => Self::Fuel,
            PitCommandMode::LfTire => Self::LfTire,
            PitCommandMode::RfTire => Self::RfTire,
            PitCommandMode::LrTire => Self::LrTire,
            PitCommandMode::RrTire => Self::RrTire,
            PitCommandMode::ClearTires => Self::ClearTires,
            PitCommandMode::FastRepair => Self::FastRepair,
            PitCommandMode::ClearTearOff => Self::ClearTearOff,
            PitCommandMode::ClearFastRepair => Self::ClearFastRepair,
            PitCommandMode::ClearFuel => Self::ClearFuel,
        }
    }
}

impl From<ReplayPositionMode> for ProtoReplayPositionMode {
    fn from(mode: ReplayPositionMode) -> Self {
        match mode {
            ReplayPositionMode::Begin => Self::Begin,
            ReplayPositionMode::Current => Self::Current,
            ReplayPositionMode::End => Self::End,
        }
    }
}

impl From<ReplaySearchMode> for ProtoReplaySearchMode {
    fn from(mode: ReplaySearchMode) -> Self {
        match mode {
            ReplaySearchMode::ToStart => Self::ToStart,
            ReplaySearchMode::ToEnd => Self::ToEnd,
            ReplaySearchMode::PreviousSession => Self::PreviousSession,
            ReplaySearchMode::NextSession => Self::NextSession,
            ReplaySearchMode::PreviousLap => Self::PreviousLap,
            ReplaySearchMode::NextLap => Self::NextLap,
            ReplaySearchMode::PreviousFrame => Self::PreviousFrame,
            ReplaySearchMode::NextFrame => Self::NextFrame,
            ReplaySearchMode::PreviousIncident => Self::PreviousIncident,
            ReplaySearchMode::NextIncident => Self::NextIncident,
        }
    }
}

impl From<ReplayStateMode> for ProtoReplayStateMode {
    fn from(mode: ReplayStateMode) -> Self {
        match mode {
            ReplayStateMode::EraseTape => Self::EraseTape,
        }
    }
}

impl From<TelemetryCommandMode> for ProtoTelemetryCommandMode {
    fn from(mode: TelemetryCommandMode) -> Self {
        match mode {
            TelemetryCommandMode::Stop => Self::Stop,
            TelemetryCommandMode::Start => Self::Start,
            TelemetryCommandMode::Restart => Self::Restart,
        }
    }
}

impl From<VideoCaptureMode> for ProtoVideoCaptureMode {
    fn from(mode: VideoCaptureMode) -> Self {
        match mode {
            VideoCaptureMode::Screenshot => Self::Screenshot,
            VideoCaptureMode::Start => Self::Start,
            VideoCaptureMode::Stop => Self::Stop,
            VideoCaptureMode::Toggle => Self::Toggle,
            VideoCaptureMode::ShowTimer => Self::ShowTimer,
            VideoCaptureMode::HideTimer => Self::HideTimer,
        }
    }
}

impl From<ProtoCameraSwitchPositionResponse> for CameraSwitchPositionResponse {
    fn from(response: ProtoCameraSwitchPositionResponse) -> Self {
        Self {
            car_index: response.car_index,
            group: response.group,
            camera: response.camera,
        }
    }
}

impl From<ProtoCameraSwitchNumberResponse> for CameraSwitchNumberResponse {
    fn from(response: ProtoCameraSwitchNumberResponse) -> Self {
        Self {
            car_index: response.car_index,
            group: response.group,
            camera: response.camera,
        }
    }
}

impl From<ProtoCameraSetStateResponse> for CameraSetStateResponse {
    fn from(response: ProtoCameraSetStateResponse) -> Self {
        Self {
            state: response.state,
        }
    }
}

impl From<ProtoReplaySetPlaySpeedResponse> for ReplaySetPlaySpeedResponse {
    fn from(response: ProtoReplaySetPlaySpeedResponse) -> Self {
        Self {
            speed: response.speed,
            is_slow_motion: response.is_slow_motion,
        }
    }
}

impl From<ProtoReplaySetPlayPositionResponse> for ReplaySetPlayPositionResponse {
    fn from(response: ProtoReplaySetPlayPositionResponse) -> Self {
        Self {
            frame: response.frame,
        }
    }
}

impl From<ProtoReplaySearchResponse> for ReplaySearchResponse {
    fn from(response: ProtoReplaySearchResponse) -> Self {
        Self {
            frame: response.frame,
            session_number: response.session_number,
            session_time: response.session_time,
        }
    }
}

impl From<ProtoPitCommandResponse> for PitCommandResponse {
    fn from(response: ProtoPitCommandResponse) -> Self {
        Self {
            service_flags: response.service_flags,
            fuel: response.fuel,
            lf_pressure: response.lf_pressure,
            rf_pressure: response.rf_pressure,
            lr_pressure: response.lr_pressure,
            rr_pressure: response.rr_pressure,
            tire_compound: response.tire_compound,
        }
    }
}

impl From<ProtoTelemetryCommandResponse> for TelemetryCommandResponse {
    fn from(response: ProtoTelemetryCommandResponse) -> Self {
        Self {
            is_disk_logging_enabled: response.is_disk_logging_enabled,
            is_disk_logging_active: response.is_disk_logging_active,
        }
    }
}

impl From<ProtoForceFeedbackCommandResponse> for ForceFeedbackCommandResponse {
    fn from(response: ProtoForceFeedbackCommandResponse) -> Self {
        Self {
            max_force: response.max_force,
        }
    }
}

impl From<ProtoCameraDetail> for CameraDetail {
    fn from(detail: ProtoCameraDetail) -> Self {
        Self {
            number: detail.number,
            name: detail.name,
        }
    }
}

impl From<ProtoCameraGroup> for CameraGroup {
    fn from(group: ProtoCameraGroup) -> Self {
        Self {
            number: group.number,
            name: group.name,
            cameras: group.cameras.into_iter().map(CameraDetail::from).collect(),
        }
    }
}

impl From<ProtoGetAvailableCamerasResponse> for GetAvailableCamerasResponse {
    fn from(response: ProtoGetAvailableCamerasResponse) -> Self {
        Self {
            camera_groups: response
                .camera_groups
                .into_iter()
                .map(CameraGroup::from)
                .collect(),
            car_index: response.car_index,
            group: response.group,
            camera: response.camera,
        }
    }
}
