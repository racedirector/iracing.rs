import { useEffect, useState } from "react";
import {
  CameraSetStateRequestForm,
  CameraSwitchNumberRequestForm,
  CameraSwitchPositionRequestForm,
  ChatCommandRequestForm,
  ForceFeedbackCommandRequestForm,
  GetAvailableCamerasRequestForm,
  PitCommandRequestForm,
  ReloadTexturesRequestForm,
  ReplaySearchRequestForm,
  ReplaySearchSessionTimeRequestForm,
  ReplaySetPlayPositionRequestForm,
  ReplaySetPlaySpeedRequestForm,
  ReplaySetStateRequestForm,
  TelemetryCommandRequestForm,
  VideoCaptureRequestForm,
} from "../components/BroadcastClientForms";
import { useBroadcastClient } from "../contexts/BroadcastClientContext";
import { formatTransportStatus } from "../server";

type BroadcastAction = {
  description: string;
  id: BroadcastActionId;
  label: string;
};

const broadcastActions: BroadcastAction[] = [
  {
    description: "Fetch available camera groups and current camera state.",
    id: "get-available-cameras",
    label: "GetAvailableCameras",
  },
  {
    description: "Switch the camera by position, group, and camera ids.",
    id: "camera-switch-position",
    label: "CameraSwitchPositionRequest",
  },
  {
    description: "Switch the camera by car number, group, and camera ids.",
    id: "camera-switch-number",
    label: "CameraSwitchNumberRequest",
  },
  {
    description: "Set camera state flags.",
    id: "camera-set-state",
    label: "CameraSetStateRequest",
  },
  {
    description: "Set replay playback speed and slow-motion mode.",
    id: "replay-set-play-speed",
    label: "ReplaySetPlaySpeedRequest",
  },
  {
    description: "Set replay position by mode and frame.",
    id: "replay-set-play-position",
    label: "ReplaySetPlayPositionRequest",
  },
  {
    description: "Search replay by replay search mode.",
    id: "replay-search",
    label: "ReplaySearchRequest",
  },
  {
    description: "Set replay state.",
    id: "replay-set-state",
    label: "ReplaySetStateRequest",
  },
  {
    description: "Reload all textures or one car texture set.",
    id: "reload-textures",
    label: "ReloadTexturesRequest",
  },
  {
    description: "Send a chat command or macro.",
    id: "chat-command",
    label: "ChatCommandRequest",
  },
  {
    description: "Send a pit-service command mode and optional value.",
    id: "pit-command",
    label: "PitCommandRequest",
  },
  {
    description: "Start, stop, or restart telemetry recording.",
    id: "telemetry-command",
    label: "TelemetryCommandRequest",
  },
  {
    description: "Set force feedback command values.",
    id: "force-feedback-command",
    label: "ForceFeedbackCommandRequest",
  },
  {
    description: "Search replay by session number and session time.",
    id: "replay-search-session-time",
    label: "ReplaySearchSessionTimeRequest",
  },
  {
    description: "Send video capture commands.",
    id: "video-capture",
    label: "VideoCaptureRequest",
  },
];

export function BroadcastClientScreen() {
  const broadcastClient = useBroadcastClient();
  const { error, grpcStatus, lastResponse, refreshStatus } = broadcastClient;
  const [activeActionId, setActiveActionId] = useState<BroadcastActionId>(
    "get-available-cameras",
  );
  const activeAction =
    broadcastActions.find((action) => action.id === activeActionId) ??
    broadcastActions[0];

  useEffect(() => {
    refreshStatus().catch(() => {});
  }, [refreshStatus]);

  return (
    <section
      className="broadcast-client-screen"
      aria-labelledby="broadcast-client-screen-title"
    >
      <div className="broadcast-client-screen__header">
        <h2 id="broadcast-client-screen-title">Broadcast Client</h2>
        <p>Exercise gRPC broadcast messages against the in-process server.</p>
        <div className={`server-runtime server-runtime--${grpcStatus.kind}`}>
          <span>gRPC {formatTransportStatus(grpcStatus)}</span>
        </div>
      </div>

      <div className="broadcast-client-layout">
        <nav
          className="broadcast-action-list"
          aria-label="Broadcast messages"
        >
          {broadcastActions.map((action) => (
            <button
              aria-current={activeActionId === action.id ? "page" : undefined}
              className={
                activeActionId === action.id
                  ? "broadcast-action-list__item broadcast-action-list__item--active"
                  : "broadcast-action-list__item"
              }
              key={action.id}
              onClick={() => setActiveActionId(action.id)}
              type="button"
            >
              <strong>{action.label}</strong>
              <span>{action.description}</span>
            </button>
          ))}
        </nav>

        <div className="broadcast-client-panel">
          <div className="broadcast-client-panel__heading">
            <h3>{activeAction.label}</h3>
            <p>{activeAction.description}</p>
          </div>

          {renderBroadcastForm(activeActionId, broadcastClient)}

          {error ? (
            <p className="broadcast-client-error" role="alert">
              {error}
            </p>
          ) : null}

          {lastResponse ? (
            <div className="broadcast-request-preview" role="status">
              <h4>Last response</h4>
              <pre>{JSON.stringify(lastResponse, null, 2)}</pre>
            </div>
          ) : null}
        </div>
      </div>
    </section>
  );
}

function renderBroadcastForm(
  actionId: BroadcastActionId,
  broadcastClient: ReturnType<typeof useBroadcastClient>,
) {
  const { isSending } = broadcastClient;
  const submitLabel = "Send request";

  switch (actionId) {
    case "get-available-cameras":
      return (
        <GetAvailableCamerasRequestForm
          isSubmitting={isSending}
          onSubmit={() => {
            broadcastClient.sendGetAvailableCamerasRequest().catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "camera-switch-position":
      return (
        <CameraSwitchPositionRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient
              .sendCameraSwitchPositionRequest(values)
              .catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "camera-switch-number":
      return (
        <CameraSwitchNumberRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient.sendCameraSwitchNumberRequest(values).catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "camera-set-state":
      return (
        <CameraSetStateRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient.sendCameraSetStateRequest(values).catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "replay-set-play-speed":
      return (
        <ReplaySetPlaySpeedRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient
              .sendReplaySetPlaySpeedRequest(values)
              .catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "replay-set-play-position":
      return (
        <ReplaySetPlayPositionRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient
              .sendReplaySetPlayPositionRequest(values)
              .catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "replay-search":
      return (
        <ReplaySearchRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient.sendReplaySearchRequest(values).catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "replay-set-state":
      return (
        <ReplaySetStateRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient.sendReplaySetStateRequest(values).catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "reload-textures":
      return (
        <ReloadTexturesRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient.sendReloadTexturesRequest(values).catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "chat-command":
      return (
        <ChatCommandRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient.sendChatCommandRequest(values).catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "pit-command":
      return (
        <PitCommandRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient.sendPitCommandRequest(values).catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "telemetry-command":
      return (
        <TelemetryCommandRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient.sendTelemetryCommandRequest(values).catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "force-feedback-command":
      return (
        <ForceFeedbackCommandRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient
              .sendForceFeedbackCommandRequest(values)
              .catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "replay-search-session-time":
      return (
        <ReplaySearchSessionTimeRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient
              .sendReplaySearchSessionTimeRequest(values)
              .catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
    case "video-capture":
      return (
        <VideoCaptureRequestForm
          isSubmitting={isSending}
          onSubmit={(values) => {
            broadcastClient.sendVideoCaptureRequest(values).catch(() => {});
          }}
          submitLabel={submitLabel}
        />
      );
  }
}

type BroadcastActionId =
  | "camera-set-state"
  | "camera-switch-number"
  | "camera-switch-position"
  | "chat-command"
  | "force-feedback-command"
  | "get-available-cameras"
  | "pit-command"
  | "reload-textures"
  | "replay-search"
  | "replay-search-session-time"
  | "replay-set-play-position"
  | "replay-set-play-speed"
  | "replay-set-state"
  | "telemetry-command"
  | "video-capture";
