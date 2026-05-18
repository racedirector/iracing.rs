export type ConnectionStatus = "disconnected" | "checking" | "connected";

export type IRacingConnectionState = {
  process: ConnectionStatus;
  sim: ConnectionStatus;
  telemetry: ConnectionStatus;
};

export type StatusIndicator = {
  label: string;
  status: ConnectionStatus;
};

export const CONNECTION_STATE_CHANGED_EVENT =
  "iracing://connection-state-changed";

export const initialConnectionState: IRacingConnectionState = {
  process: "checking",
  sim: "disconnected",
  telemetry: "disconnected",
};

export const disconnectedConnectionState: IRacingConnectionState = {
  process: "disconnected",
  sim: "disconnected",
  telemetry: "disconnected",
};

export function getConnectionStatus(
  connectionState: IRacingConnectionState,
): ConnectionStatus {
  const statuses = Object.values(connectionState);

  if (statuses.every((status) => status === "connected")) {
    return "connected";
  }

  if (statuses.every((status) => status === "disconnected")) {
    return "disconnected";
  }

  return "checking";
}

export function getLifecycleStatus(
  connectionState: IRacingConnectionState,
): StatusIndicator[] {
  return [
    { label: "iRacing Process", status: connectionState.process },
    { label: "Sim Status", status: connectionState.sim },
    { label: "Telemetry", status: connectionState.telemetry },
  ];
}

export function formatStatus(status: ConnectionStatus) {
  return status[0].toUpperCase() + status.slice(1);
}
