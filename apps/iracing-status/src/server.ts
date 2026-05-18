export type ServerGeneralSettings = {
  httpEnabled: boolean;
  websocketEnabled: boolean;
  grpcEnabled: boolean;
};

export type ServerDataSourceSettings =
  | { kind: "live" }
  | {
      kind: "ibtFile";
      fileName: string;
      fileSize: number;
      lastModified: number;
    };

export type TransportSettings = {
  host: string;
  port: number;
};

export type ServerSettings = {
  dataSource: ServerDataSourceSettings;
  general: ServerGeneralSettings;
  http: TransportSettings;
  websocket: TransportSettings;
  grpc: TransportSettings;
};

export type TransportRuntimeStatus =
  | { kind: "disabled" }
  | { kind: "running"; endpoint: string };

export type ServerRuntimeStatus = {
  http: TransportRuntimeStatus;
  websocket: TransportRuntimeStatus;
  grpc: TransportRuntimeStatus;
};

export type ServerState = {
  settings: ServerSettings;
  status: ServerRuntimeStatus;
};

export type TransportKey = "http" | "websocket" | "grpc";

export const defaultServerSettings: ServerSettings = {
  dataSource: {
    kind: "live",
  },
  general: {
    httpEnabled: false,
    websocketEnabled: false,
    grpcEnabled: false,
  },
  http: {
    host: "127.0.0.1",
    port: 32080,
  },
  websocket: {
    host: "127.0.0.1",
    port: 32081,
  },
  grpc: {
    host: "127.0.0.1",
    port: 32082,
  },
};

export const defaultServerState: ServerState = {
  settings: defaultServerSettings,
  status: {
    http: { kind: "disabled" },
    websocket: { kind: "disabled" },
    grpc: { kind: "disabled" },
  },
};

export function getTransportEnabled(
  settings: ServerSettings,
  transport: TransportKey,
) {
  switch (transport) {
    case "http":
      return settings.general.httpEnabled;
    case "websocket":
      return settings.general.websocketEnabled;
    case "grpc":
      return settings.general.grpcEnabled;
  }
}

export function setTransportEnabled(
  settings: ServerSettings,
  transport: TransportKey,
  enabled: boolean,
): ServerSettings {
  return {
    ...settings,
    general: {
      ...settings.general,
      [`${transport}Enabled`]: enabled,
    },
  };
}

export function formatTransportStatus(status: TransportRuntimeStatus) {
  switch (status.kind) {
    case "disabled":
      return "Disabled";
    case "running":
      return `Running at ${status.endpoint}`;
  }
}
