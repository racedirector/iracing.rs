import { invoke } from "@tauri-apps/api/core";
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { type TransportRuntimeStatus } from "../server";
import { useServerState } from "./ServerStateContext";

const uint32Max = 4_294_967_295;
const int32Min = -2_147_483_648;
const int32Max = 2_147_483_647;

type StringValues = Record<string, string>;

type BroadcastClientRequest =
  | { message: "CameraSetStateRequest"; values: { state: number } }
  | {
      message: "CameraSwitchNumberRequest";
      values: { camera: number; carNumber: string; group: number };
    }
  | {
      message: "CameraSwitchPositionRequest";
      values: { camera: number; group: number; position: number };
    }
  | {
      message: "ChatCommandRequest";
      values: { macro?: number; mode: string };
    }
  | {
      message: "ForceFeedbackCommandRequest";
      values: { mode: string; value: number };
    }
  | { message: "GetAvailableCamerasRequest"; values: Record<string, never> }
  | {
      message: "PitCommandRequest";
      values: { mode: string; value?: number };
    }
  | { message: "ReloadTexturesRequest"; values: { carIdx?: number } }
  | { message: "ReplaySearchRequest"; values: { mode: string } }
  | {
      message: "ReplaySearchSessionTimeRequest";
      values: { sessionNumber: number; sessionTimeMs: number };
    }
  | {
      message: "ReplaySetPlayPositionRequest";
      values: { frame: number; mode: string };
    }
  | {
      message: "ReplaySetPlaySpeedRequest";
      values: { isSlowMotion: boolean; speed: number };
    }
  | { message: "ReplaySetStateRequest"; values: { state: string } }
  | { message: "TelemetryCommandRequest"; values: { mode: string } }
  | { message: "VideoCaptureRequest"; values: { mode: string } };

type BroadcastClientResponse = {
  message: string;
  values: unknown;
};

type BroadcastClientContextValue = {
  error: string | null;
  grpcStatus: TransportRuntimeStatus;
  isSending: boolean;
  lastResponse: BroadcastClientResponse | null;
  refreshStatus: () => Promise<void>;
  sendBroadcastClientRequest: (
    request: BroadcastClientRequest,
  ) => Promise<BroadcastClientResponse>;
  sendCameraSetStateRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendCameraSwitchNumberRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendCameraSwitchPositionRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendChatCommandRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendForceFeedbackCommandRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendGetAvailableCamerasRequest: () => Promise<BroadcastClientResponse>;
  sendPitCommandRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendReloadTexturesRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendReplaySearchRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendReplaySearchSessionTimeRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendReplaySetPlayPositionRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendReplaySetPlaySpeedRequest: (values: {
    isSlowMotion: boolean;
    speed: string;
  }) => Promise<BroadcastClientResponse>;
  sendReplaySetStateRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendTelemetryCommandRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
  sendVideoCaptureRequest: (
    values: StringValues,
  ) => Promise<BroadcastClientResponse>;
};

const BroadcastClientContext =
  createContext<BroadcastClientContextValue | null>(null);

type BroadcastClientProviderProps = {
  children: ReactNode;
};

export function BroadcastClientProvider({
  children,
}: BroadcastClientProviderProps) {
  const { refreshServerState, serverState } = useServerState();
  const [isSending, setIsSending] = useState(false);
  const [lastResponse, setLastResponse] =
    useState<BroadcastClientResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshStatus = useCallback(async () => {
    await refreshServerState();
  }, [refreshServerState]);

  const sendBroadcastClientRequest = useCallback(
    async (request: BroadcastClientRequest) => {
      setIsSending(true);
      setError(null);

      try {
        const nextState = await refreshServerState();
        if (nextState.status.grpc.kind !== "running") {
          throw new Error("gRPC service is not running.");
        }

        const response = await invoke<BroadcastClientResponse>(
          "send_broadcast_client_request",
          { request },
        );
        setLastResponse(response);
        return response;
      } catch (sendError) {
        const message = formatError(sendError);
        setError(message);
        throw new Error(message);
      } finally {
        setIsSending(false);
      }
    },
    [refreshServerState],
  );

  const sendCameraSetStateRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "CameraSetStateRequest",
          values: { state: requiredUint32(values.state, "State") },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendCameraSwitchNumberRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "CameraSwitchNumberRequest",
          values: {
            camera: requiredUint32(values.camera, "Camera"),
            carNumber: requiredText(values.carNumber, "Car Number"),
            group: requiredUint32(values.group, "Group"),
          },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendCameraSwitchPositionRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "CameraSwitchPositionRequest",
          values: {
            camera: requiredUint32(values.camera, "Camera"),
            group: requiredUint32(values.group, "Group"),
            position: requiredUint32(values.position, "Position"),
          },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendChatCommandRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "ChatCommandRequest",
          values: {
            macro: optionalUint32(values.macro, "Macro"),
            mode: requiredMode(values.mode, "Mode"),
          },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendForceFeedbackCommandRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "ForceFeedbackCommandRequest",
          values: {
            mode: requiredMode(values.mode, "Mode"),
            value: requiredFiniteNumber(values.value, "Value"),
          },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendGetAvailableCamerasRequest = useCallback(
    () =>
      sendBroadcastClientRequest({
        message: "GetAvailableCamerasRequest",
        values: {},
      }),
    [sendBroadcastClientRequest],
  );

  const sendPitCommandRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "PitCommandRequest",
          values: {
            mode: requiredMode(values.mode, "Mode"),
            value: optionalFiniteNumber(values.value, "Value"),
          },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendReloadTexturesRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "ReloadTexturesRequest",
          values: { carIdx: optionalUint32(values.carIdx, "Car Index") },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendReplaySearchRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "ReplaySearchRequest",
          values: { mode: requiredMode(values.mode, "Mode") },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendReplaySearchSessionTimeRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "ReplaySearchSessionTimeRequest",
          values: {
            sessionNumber: requiredUint32(
              values.sessionNumber,
              "Session Number",
            ),
            sessionTimeMs: requiredUint32(
              values.sessionTimeMs,
              "Session Time Ms",
            ),
          },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendReplaySetPlayPositionRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "ReplaySetPlayPositionRequest",
          values: {
            frame: requiredUint32(values.frame, "Frame"),
            mode: requiredMode(values.mode, "Mode"),
          },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendReplaySetPlaySpeedRequest = useCallback(
    (values: { isSlowMotion: boolean; speed: string }) =>
      sendMappedRequest(
        () => ({
          message: "ReplaySetPlaySpeedRequest",
          values: {
            isSlowMotion: values.isSlowMotion,
            speed: requiredInt32(values.speed, "Speed"),
          },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendReplaySetStateRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "ReplaySetStateRequest",
          values: { state: requiredMode(values.state, "State") },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendTelemetryCommandRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "TelemetryCommandRequest",
          values: { mode: requiredMode(values.mode, "Mode") },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const sendVideoCaptureRequest = useCallback(
    (values: StringValues) =>
      sendMappedRequest(
        () => ({
          message: "VideoCaptureRequest",
          values: { mode: requiredMode(values.mode, "Mode") },
        }),
        sendBroadcastClientRequest,
        setError,
      ),
    [sendBroadcastClientRequest],
  );

  const contextValue = useMemo(
    () => ({
      error,
      grpcStatus: serverState.status.grpc,
      isSending,
      lastResponse,
      refreshStatus,
      sendBroadcastClientRequest,
      sendCameraSetStateRequest,
      sendCameraSwitchNumberRequest,
      sendCameraSwitchPositionRequest,
      sendChatCommandRequest,
      sendForceFeedbackCommandRequest,
      sendGetAvailableCamerasRequest,
      sendPitCommandRequest,
      sendReloadTexturesRequest,
      sendReplaySearchRequest,
      sendReplaySearchSessionTimeRequest,
      sendReplaySetPlayPositionRequest,
      sendReplaySetPlaySpeedRequest,
      sendReplaySetStateRequest,
      sendTelemetryCommandRequest,
      sendVideoCaptureRequest,
    }),
    [
      error,
      serverState.status.grpc,
      isSending,
      lastResponse,
      refreshStatus,
      sendBroadcastClientRequest,
      sendCameraSetStateRequest,
      sendCameraSwitchNumberRequest,
      sendCameraSwitchPositionRequest,
      sendChatCommandRequest,
      sendForceFeedbackCommandRequest,
      sendGetAvailableCamerasRequest,
      sendPitCommandRequest,
      sendReloadTexturesRequest,
      sendReplaySearchRequest,
      sendReplaySearchSessionTimeRequest,
      sendReplaySetPlayPositionRequest,
      sendReplaySetPlaySpeedRequest,
      sendReplaySetStateRequest,
      sendTelemetryCommandRequest,
      sendVideoCaptureRequest,
    ],
  );

  return (
    <BroadcastClientContext.Provider value={contextValue}>
      {children}
    </BroadcastClientContext.Provider>
  );
}

export function useBroadcastClient() {
  const broadcastClient = useContext(BroadcastClientContext);

  if (!broadcastClient) {
    throw new Error(
      "useBroadcastClient must be used within BroadcastClientProvider",
    );
  }

  return broadcastClient;
}

async function sendMappedRequest(
  buildRequest: () => BroadcastClientRequest,
  sendRequest: (
    request: BroadcastClientRequest,
  ) => Promise<BroadcastClientResponse>,
  setError: (message: string) => void,
) {
  try {
    return await sendRequest(buildRequest());
  } catch (sendError) {
    const message = formatError(sendError);
    setError(message);
    throw new Error(message);
  }
}

function requiredMode(value: string, label: string) {
  if (value === "unset" || !value) {
    throw new Error(`Select ${label.toLowerCase()}.`);
  }

  return value;
}

function requiredText(value: string, label: string) {
  const trimmedValue = value.trim();
  if (!trimmedValue) {
    throw new Error(`${label} is required.`);
  }

  return trimmedValue;
}

function requiredUint32(value: string, label: string) {
  const trimmedValue = requiredText(value, label);
  if (!/^\d+$/.test(trimmedValue)) {
    throw new Error(`${label} must use digits only.`);
  }

  const parsedValue = Number(trimmedValue);
  if (parsedValue > uint32Max) {
    throw new Error(`${label} must be at most ${uint32Max}.`);
  }

  return parsedValue;
}

function optionalUint32(value: string | undefined, label: string) {
  if (!value?.trim()) {
    return undefined;
  }

  return requiredUint32(value, label);
}

function requiredInt32(value: string, label: string) {
  const trimmedValue = requiredText(value, label);
  if (!/^-?\d+$/.test(trimmedValue)) {
    throw new Error(`${label} must be an integer.`);
  }

  const parsedValue = Number(trimmedValue);
  if (parsedValue < int32Min || parsedValue > int32Max) {
    throw new Error(`${label} must be from ${int32Min} to ${int32Max}.`);
  }

  return parsedValue;
}

function requiredFiniteNumber(value: string, label: string) {
  const parsedValue = Number(requiredText(value, label));
  if (!Number.isFinite(parsedValue)) {
    throw new Error(`${label} must be finite.`);
  }

  return parsedValue;
}

function optionalFiniteNumber(value: string | undefined, label: string) {
  if (!value?.trim()) {
    return undefined;
  }

  return requiredFiniteNumber(value, label);
}

function formatError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

export type { BroadcastClientRequest, BroadcastClientResponse };
