import {
  ReloadTexturesRequest,
  CameraSwitchPositionRequest,
  CameraSwitchNumberRequest,
  CameraSetStateRequest,
  PitCommandRequest,
  PitCommandMode as GRPCPitCommandMode,
  ReplaySetPlaySpeedRequest,
  ReplaySetPlayPositionRequest,
  ReplayPositionMode as GRPCReplayPositionMode,
  ReplaySearchRequest,
  ReplaySearchMode as GRPCReplaySearchMode,
  ReplaySetStateRequest,
  ReplayStateMode as GRPCReplayStateMode,
  ChatCommandRequest,
  ChatCommandMode as GRPCChatCommandMode,
  TelemetryCommandRequest,
  TelemetryCommandMode as GRPCTelemetryCommandMode,
  ForceFeedbackCommandMode,
  VideoCaptureMode,
  ForceFeedbackCommandRequest,
  ReplaySearchSessionTimeRequest,
  VideoCaptureRequest,
} from "../../generated/grpc-web/broadcast_pb";
import { BroadcastPromiseClient } from "../../generated/grpc-web/broadcast_grpc_web_pb";
import {
  BroadcastClientAPI,
  ChatCommandMode,
  PitCommandMode,
  ReplayPositionMode,
  ReplaySearchMode,
  ReplayStateMode,
  TelemetryCommandMode,
  VideoCaptureCommandMode,
} from "../../contexts/BroadcastClient";

export class BroadcastClient implements BroadcastClientAPI {
  private _client: BroadcastPromiseClient;
  get client() {
    return this._client;
  }

  constructor(url: string) {
    this._client = new BroadcastPromiseClient(url);
  }

  async reloadTextures(carIndex?: number) {
    const request = new ReloadTexturesRequest();
    if (carIndex !== undefined) {
      request.setCarIdx(carIndex);
    }

    await this.client.reloadTextures(request);
  }

  async switchCameraPosition(position: number, group: number, camera: number) {
    const request = new CameraSwitchPositionRequest();
    request.setPosition(position);
    request.setGroup(group);
    request.setCamera(camera);

    const result = await this.client.cameraSwitchPosition(request);

    return result.toObject();
  }

  async switchCameraNumber(number: string, group: number, camera: number) {
    const request = new CameraSwitchNumberRequest();
    request.setCarNumber(number);
    request.setGroup(group);
    request.setCamera(camera);

    const result = await this.client.cameraSwitchNumber(request);
    return result.toObject();
  }

  async setCameraState(state?: number) {
    const request = new CameraSetStateRequest();
    if (state !== undefined) {
      request.setState(state);
    }

    const result = await this.client.cameraSetState(request);
    return result.toObject();
  }

  async sendPitCommand(mode: PitCommandMode, value?: number) {
    const request = new PitCommandRequest();
    switch (mode) {
      case "clear":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_CLEAR);
        break;
      case "tear-off":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_TEAR_OFF);
        break;
      case "fuel":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_FUEL);
        break;
      case "lf-tire":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_LF_TIRE);
        break;
      case "rf-tire":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_RF_TIRE);
        break;
      case "lr-tire":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_LR_TIRE);
        break;
      case "rr-tire":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_RR_TIRE);
        break;
      case "clear-tires":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_CLEAR_TIRES);
        break;
      case "fast-repair":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_FAST_REPAIR);
        break;
      case "clear-tear-off":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_CLEAR_TEAR_OFF);
        break;
      case "clear-fast-repair":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_CLEAR_FAST_REPAIR);
        break;
      case "clear-fuel":
        request.setMode(GRPCPitCommandMode.PIT_COMMAND_MODE_CLEAR_FUEL);
    }

    if (value !== undefined) {
      request.setValue(value);
    }

    const result = await this.client.pitCommand(request);
    return result.toObject();
  }

  async setPlaySpeed(speed: number, isSlowMotion?: boolean) {
    const request = new ReplaySetPlaySpeedRequest();
    request.setSpeed(speed);
    if (isSlowMotion !== undefined) {
      request.setIsSlowMotion(isSlowMotion);
    }

    const result = await this.client.replaySetPlaySpeed(request);

    return result.toObject();
  }

  async setPlayPosition(mode: ReplayPositionMode, frame: number) {
    const request = new ReplaySetPlayPositionRequest();
    switch (mode) {
      case "begin":
        request.setMode(GRPCReplayPositionMode.REPLAY_POSITION_MODE_BEGIN);
        break;
      case "current":
        request.setMode(GRPCReplayPositionMode.REPLAY_POSITION_MODE_CURRENT);
        break;
      case "end":
        request.setMode(GRPCReplayPositionMode.REPLAY_POSITION_MODE_END);
        break;
    }

    request.setFrame(frame);

    const result = await this.client.replaySetPlayPosition(request);
    return result.toObject();
  }

  async searchReplay(mode: ReplaySearchMode) {
    const request = new ReplaySearchRequest();
    switch (mode) {
      case "to-start":
        request.setMode(GRPCReplaySearchMode.REPLAY_SEARCH_MODE_TO_START);
        break;
      case "to-end":
        request.setMode(GRPCReplaySearchMode.REPLAY_SEARCH_MODE_TO_END);
        break;
      case "previous-session":
        request.setMode(
          GRPCReplaySearchMode.REPLAY_SEARCH_MODE_PREVIOUS_SESSION,
        );
        break;
      case "next-session":
        request.setMode(GRPCReplaySearchMode.REPLAY_SEARCH_MODE_NEXT_SESSION);
        break;
      case "previous-lap":
        request.setMode(GRPCReplaySearchMode.REPLAY_SEARCH_MODE_PREVIOUS_LAP);
        break;
      case "next-lap":
        request.setMode(GRPCReplaySearchMode.REPLAY_SEARCH_MODE_NEXT_LAP);
        break;
      case "previous-frame":
        request.setMode(GRPCReplaySearchMode.REPLAY_SEARCH_MODE_PREVIOUS_FRAME);
        break;
      case "next-frame":
        request.setMode(GRPCReplaySearchMode.REPLAY_SEARCH_MODE_NEXT_FRAME);
        break;
      case "previous-incident":
        request.setMode(
          GRPCReplaySearchMode.REPLAY_SEARCH_MODE_PREVIOUS_INCIDENT,
        );
        break;
      case "next-incident":
        request.setMode(GRPCReplaySearchMode.REPLAY_SEARCH_MODE_NEXT_INCIDENT);
        break;
    }

    const result = await this.client.replaySearch(request);
    return result.toObject();
  }

  async setReplayState(mode: ReplayStateMode) {
    const request = new ReplaySetStateRequest();
    switch (mode) {
      case "erase-tape":
        request.setState(GRPCReplayStateMode.REPLAY_STATE_MODE_ERASE_TAPE);
        break;
    }

    await this.client.replaySetState(request);
  }

  async chatCommand(mode: ChatCommandMode, macro?: number) {
    const request = new ChatCommandRequest();
    switch (mode) {
      case "macro":
        request.setMode(GRPCChatCommandMode.CHAT_COMMAND_MODE_MACRO);
        break;
      case "begin-chat":
        request.setMode(GRPCChatCommandMode.CHAT_COMMAND_MODE_BEGIN_CHAT);
        break;
      case "reply":
        request.setMode(GRPCChatCommandMode.CHAT_COMMAND_MODE_REPLY);
        break;
      case "cancel":
        request.setMode(GRPCChatCommandMode.CHAT_COMMAND_MODE_CANCEL);
        break;
    }

    if (macro !== undefined) {
      request.setMacro(macro);
    }

    await this.client.chatCommand(request);
  }

  async telemetryCommand(mode: TelemetryCommandMode) {
    const request = new TelemetryCommandRequest();
    switch (mode) {
      case "stop":
        request.setMode(GRPCTelemetryCommandMode.TELEMETRY_COMMAND_MODE_STOP);
        break;
      case "start":
        request.setMode(GRPCTelemetryCommandMode.TELEMETRY_COMMAND_MODE_START);
        break;
      case "restart":
        request.setMode(
          GRPCTelemetryCommandMode.TELEMETRY_COMMAND_MODE_RESTART,
        );
        break;
    }

    const result = await this.client.telemetryCommand(request);
    return result.toObject();
  }

  async ffbCommand(value?: number) {
    const request = new ForceFeedbackCommandRequest();

    request.setMode(
      ForceFeedbackCommandMode.FORCE_FEEDBACK_COMMAND_MODE_MAX_FORCE,
    );

    if (value !== undefined) {
      request.setValue(value);
    }

    const result = await this.client.forceFeedbackCommand(request);

    return result.toObject();
  }

  async replaySearchSessionTime(sessionNumber: number, sessionTimeMs: number) {
    const request = new ReplaySearchSessionTimeRequest();
    request.setSessionNumber(sessionNumber);
    request.setSessionTimeMs(sessionTimeMs);

    await this.client.replaySearchSessionTime(request);
  }

  async videoCaptureCommand(mode: VideoCaptureCommandMode) {
    const request = new VideoCaptureRequest();

    switch (mode) {
      case "screenshot":
        request.setMode(VideoCaptureMode.VIDEO_CAPTURE_MODE_SCREENSHOT);
        break;
      case "start":
        request.setMode(VideoCaptureMode.VIDEO_CAPTURE_MODE_START);
        break;
      case "stop":
        request.setMode(VideoCaptureMode.VIDEO_CAPTURE_MODE_STOP);
        break;
      case "toggle":
        request.setMode(VideoCaptureMode.VIDEO_CAPTURE_MODE_TOGGLE);
        break;
      case "show-timer":
        request.setMode(VideoCaptureMode.VIDEO_CAPTURE_MODE_SHOW_TIMER);
        break;
      case "hide-timer":
        request.setMode(VideoCaptureMode.VIDEO_CAPTURE_MODE_HIDE_TIMER);
        break;
    }

    await this.client.videoCapture(request);
  }
}

export default BroadcastClient;
