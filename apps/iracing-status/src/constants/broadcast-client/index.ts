import { createClient, type Client } from "@connectrpc/connect";
import { createGrpcWebTransport } from "@connectrpc/connect-web";
import {
  Broadcast,
  ChatCommandMode as GRPCChatCommandMode,
  ForceFeedbackCommandMode,
  PitCommandMode as GRPCPitCommandMode,
  ReplayPositionMode as GRPCReplayPositionMode,
  ReplaySearchMode as GRPCReplaySearchMode,
  ReplayStateMode as GRPCReplayStateMode,
  TelemetryCommandMode as GRPCTelemetryCommandMode,
  VideoCaptureMode,
} from "../../generated/grpc-web/broadcast_pb";
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

const pitCommandModes: Record<PitCommandMode, GRPCPitCommandMode> = {
  clear: GRPCPitCommandMode.CLEAR,
  "tear-off": GRPCPitCommandMode.TEAR_OFF,
  fuel: GRPCPitCommandMode.FUEL,
  "lf-tire": GRPCPitCommandMode.LF_TIRE,
  "rf-tire": GRPCPitCommandMode.RF_TIRE,
  "lr-tire": GRPCPitCommandMode.LR_TIRE,
  "rr-tire": GRPCPitCommandMode.RR_TIRE,
  "clear-tires": GRPCPitCommandMode.CLEAR_TIRES,
  "fast-repair": GRPCPitCommandMode.FAST_REPAIR,
  "clear-tear-off": GRPCPitCommandMode.CLEAR_TEAR_OFF,
  "clear-fast-repair": GRPCPitCommandMode.CLEAR_FAST_REPAIR,
  "clear-fuel": GRPCPitCommandMode.CLEAR_FUEL,
};

const replayPositionModes: Record<ReplayPositionMode, GRPCReplayPositionMode> =
  {
    begin: GRPCReplayPositionMode.BEGIN,
    current: GRPCReplayPositionMode.CURRENT,
    end: GRPCReplayPositionMode.END,
  };

const replaySearchModes: Record<ReplaySearchMode, GRPCReplaySearchMode> = {
  "to-start": GRPCReplaySearchMode.TO_START,
  "to-end": GRPCReplaySearchMode.TO_END,
  "previous-session": GRPCReplaySearchMode.PREVIOUS_SESSION,
  "next-session": GRPCReplaySearchMode.NEXT_SESSION,
  "previous-lap": GRPCReplaySearchMode.PREVIOUS_LAP,
  "next-lap": GRPCReplaySearchMode.NEXT_LAP,
  "previous-frame": GRPCReplaySearchMode.PREVIOUS_FRAME,
  "next-frame": GRPCReplaySearchMode.NEXT_FRAME,
  "previous-incident": GRPCReplaySearchMode.PREVIOUS_INCIDENT,
  "next-incident": GRPCReplaySearchMode.NEXT_INCIDENT,
};

const replayStateModes: Record<ReplayStateMode, GRPCReplayStateMode> = {
  "erase-tape": GRPCReplayStateMode.ERASE_TAPE,
};

const chatCommandModes: Record<ChatCommandMode, GRPCChatCommandMode> = {
  macro: GRPCChatCommandMode.MACRO,
  "begin-chat": GRPCChatCommandMode.BEGIN_CHAT,
  reply: GRPCChatCommandMode.REPLY,
  cancel: GRPCChatCommandMode.CANCEL,
};

const telemetryCommandModes: Record<
  TelemetryCommandMode,
  GRPCTelemetryCommandMode
> = {
  stop: GRPCTelemetryCommandMode.STOP,
  start: GRPCTelemetryCommandMode.START,
  restart: GRPCTelemetryCommandMode.RESTART,
};

const videoCaptureModes: Record<VideoCaptureCommandMode, VideoCaptureMode> = {
  screenshot: VideoCaptureMode.SCREENSHOT,
  start: VideoCaptureMode.START,
  stop: VideoCaptureMode.STOP,
  toggle: VideoCaptureMode.TOGGLE,
  "show-timer": VideoCaptureMode.SHOW_TIMER,
  "hide-timer": VideoCaptureMode.HIDE_TIMER,
};

export class BroadcastClient implements BroadcastClientAPI {
  private _client: Client<typeof Broadcast>;
  get client() {
    return this._client;
  }

  constructor(url: string) {
    this._client = createClient(
      Broadcast,
      createGrpcWebTransport({
        baseUrl: url,
      }),
    );
  }

  async reloadTextures(carIndex?: number) {
    await this.client.reloadTextures(
      carIndex === undefined ? {} : { carIdx: carIndex },
    );
  }

  async switchCameraPosition(position: number, group: number, camera: number) {
    const result = await this.client.cameraSwitchPosition({
      position,
      group,
      camera,
    });

    return {
      carIndex: result.carIndex,
      group: result.group,
      camera: result.camera,
    };
  }

  async switchCameraNumber(number: string, group: number, camera: number) {
    const result = await this.client.cameraSwitchNumber({
      carNumber: number,
      group,
      camera,
    });

    return {
      carIndex: result.carIndex,
      group: result.group,
      camera: result.camera,
    };
  }

  async setCameraState(state?: number) {
    const result = await this.client.cameraSetState(
      state === undefined ? {} : { state },
    );

    return {
      state: result.state,
    };
  }

  async sendPitCommand(mode: PitCommandMode, value?: number) {
    const result = await this.client.pitCommand({
      mode: pitCommandModes[mode],
      value,
    });

    return {
      serviceFlags: result.serviceFlags,
      fuel: result.fuel,
      lfPressure: result.lfPressure,
      rfPressure: result.rfPressure,
      lrPressure: result.lrPressure,
      rrPressure: result.rrPressure,
      tireCompound: result.tireCompound,
    };
  }

  async setPlaySpeed(speed: number, isSlowMotion?: boolean) {
    const result = await this.client.replaySetPlaySpeed({
      speed,
      isSlowMotion,
    });

    return {
      speed: result.speed,
      isSlowMotion: result.isSlowMotion,
    };
  }

  async setPlayPosition(mode: ReplayPositionMode, frame: number) {
    const result = await this.client.replaySetPlayPosition({
      mode: replayPositionModes[mode],
      frame,
    });

    return {
      frame: result.frame,
    };
  }

  async searchReplay(mode: ReplaySearchMode) {
    const result = await this.client.replaySearch({
      mode: replaySearchModes[mode],
    });

    return {
      frame: result.frame,
      sessionNumber: result.sessionNumber,
      sessionTime: result.sessionTime,
    };
  }

  async setReplayState(mode: ReplayStateMode) {
    await this.client.replaySetState({
      state: replayStateModes[mode],
    });
  }

  async chatCommand(mode: ChatCommandMode, macro?: number) {
    await this.client.chatCommand({
      mode: chatCommandModes[mode],
      macro,
    });
  }

  async telemetryCommand(mode: TelemetryCommandMode) {
    const result = await this.client.telemetryCommand({
      mode: telemetryCommandModes[mode],
    });

    return {
      isDiskLoggingEnabled: result.isDiskLoggingEnabled,
      isDiskLoggingActive: result.isDiskLoggingActive,
    };
  }

  async ffbCommand(value?: number) {
    const result = await this.client.forceFeedbackCommand({
      mode: ForceFeedbackCommandMode.MAX_FORCE,
      value,
    });

    return {
      maxForce: result.maxForce,
    };
  }

  async replaySearchSessionTime(sessionNumber: number, sessionTimeMs: number) {
    await this.client.replaySearchSessionTime({
      sessionNumber,
      sessionTimeMs,
    });
  }

  async videoCaptureCommand(mode: VideoCaptureCommandMode) {
    await this.client.videoCapture({
      mode: videoCaptureModes[mode],
    });
  }
}

export default BroadcastClient;
