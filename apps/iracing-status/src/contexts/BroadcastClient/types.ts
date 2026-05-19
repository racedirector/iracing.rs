export type TelemetryCommandMode = "stop" | "start" | "restart";

export type ChatCommandMode = "macro" | "begin-chat" | "reply" | "cancel";

export type ReplayPositionMode = "begin" | "current" | "end";

export type ReplaySearchMode =
  | "to-start"
  | "to-end"
  | "previous-session"
  | "next-session"
  | "previous-lap"
  | "next-lap"
  | "previous-frame"
  | "next-frame"
  | "previous-incident"
  | "next-incident";

export type PitCommandMode =
  | "clear"
  | "tear-off"
  | "fuel"
  | "lf-tire"
  | "rf-tire"
  | "lr-tire"
  | "rr-tire"
  | "clear-tires"
  | "fast-repair"
  | "clear-tear-off"
  | "clear-fast-repair"
  | "clear-fuel";

export type ReplayStateMode = "erase-tape";

export type VideoCaptureCommandMode =
  | "screenshot"
  | "start"
  | "stop"
  | "toggle"
  | "show-timer"
  | "hide-timer";

export type ForceFeedbackCommandMode = "max-force";

export interface BroadcastClientAPI {
  reloadTextures(carIndex?: number): Promise<void>;

  switchCameraPosition(
    position: number,
    group: number,
    camera: number,
  ): Promise<{
    carIndex: number;
    group: number;
    camera: number;
  }>;

  switchCameraNumber(
    number: string,
    group: number,
    camera: number,
  ): Promise<{
    carIndex: number;
    group: number;
    camera: number;
  }>;

  setCameraState(state?: number): Promise<{
    state: number;
  }>;

  sendPitCommand(
    mode: PitCommandMode,
    value?: number,
  ): Promise<{
    serviceFlags: number;
    fuel: number;
    lfPressure: number;
    rfPressure: number;
    lrPressure: number;
    rrPressure: number;
    tireCompound: number;
  }>;

  setPlaySpeed(
    speed: number,
    isSlowMotion?: boolean,
  ): Promise<{
    speed: number;
    isSlowMotion: boolean;
  }>;

  setPlayPosition(
    mode: ReplayPositionMode,
    frame: number,
  ): Promise<{
    frame: number;
  }>;

  searchReplay(mode: ReplaySearchMode): Promise<{
    frame: number;
    sessionNumber: number;
    sessionTime: number;
  }>;

  setReplayState(mode: ReplayStateMode): Promise<void>;

  chatCommand(mode: ChatCommandMode, macro?: number): Promise<void>;

  telemetryCommand(mode: TelemetryCommandMode): Promise<{
    isDiskLoggingEnabled: boolean;
    isDiskLoggingActive: boolean;
  }>;

  ffbCommand(value?: number): Promise<{
    maxForce: number;
  }>;

  replaySearchSessionTime(
    sessionNumber: number,
    sessionTimeMs: number,
  ): Promise<void>;

  videoCaptureCommand(mode: VideoCaptureCommandMode): Promise<void>;
}
