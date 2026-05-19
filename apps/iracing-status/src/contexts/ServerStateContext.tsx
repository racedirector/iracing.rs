import { invoke } from "@tauri-apps/api/core";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  defaultServerState,
  type ServerDataSourceSettings,
  type ServerSettings,
  type ServerState,
  type TransportSettings,
} from "../server";

type ServerStateContextValue = {
  applyServerSettings: (settings: ServerSettings) => Promise<ServerState>;
  setDataSourceSettings: (settings: ServerDataSourceSettings) => void;
  error: string | null;
  isLoading: boolean;
  isSaving: boolean;
  refreshServerState: () => Promise<ServerState>;
  serverState: ServerState;
};

const ServerStateContext = createContext<ServerStateContextValue | null>(null);

type ServerStateProviderProps = {
  children: ReactNode;
};

export function ServerStateProvider({ children }: ServerStateProviderProps) {
  const [serverState, setServerState] =
    useState<ServerState>(defaultServerState);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshServerState = useCallback(async () => {
    try {
      const nextStateFromServer =
        await invoke<RustServerState>("get_server_state");
      const nextState = mergeRustServerState(
        nextStateFromServer,
        serverState.settings.dataSource,
      );
      setServerState(nextState);
      setError(null);
      return nextState;
    } catch (refreshError) {
      const message = formatError(refreshError);
      setError(message);
      throw new Error(message);
    }
  }, [serverState.settings.dataSource]);

  const applyServerSettings = useCallback(async (settings: ServerSettings) => {
    setIsSaving(true);
    setError(null);

    try {
      const nextStateFromServer = await invoke<RustServerState>(
        "set_server_settings",
        {
          settings: toRustServerSettings(settings),
        },
      );
      const nextState = mergeRustServerState(
        nextStateFromServer,
        settings.dataSource,
      );
      setServerState(nextState);
      return nextState;
    } catch (saveError) {
      const message = formatError(saveError);
      setError(message);
      throw new Error(message);
    } finally {
      setIsSaving(false);
    }
  }, []);

  const setDataSourceSettings = useCallback(
    (dataSource: ServerDataSourceSettings) => {
      setServerState((currentState) => ({
        ...currentState,
        settings: {
          ...currentState.settings,
          dataSource,
        },
      }));
    },
    [],
  );

  useEffect(() => {
    let isMounted = true;

    async function loadServerState() {
      try {
        const nextStateFromServer =
          await invoke<RustServerState>("get_server_state");
        if (isMounted) {
          setServerState((currentState) =>
            mergeRustServerState(
              nextStateFromServer,
              currentState.settings.dataSource,
            ),
          );
          setError(null);
        }
      } catch (loadError) {
        if (isMounted) {
          setError(formatError(loadError));
        }
      } finally {
        if (isMounted) {
          setIsLoading(false);
        }
      }
    }

    loadServerState();

    return () => {
      isMounted = false;
    };
  }, []);

  const contextValue = useMemo(
    () => ({
      applyServerSettings,
      error,
      isLoading,
      isSaving,
      refreshServerState,
      serverState,
      setDataSourceSettings,
    }),
    [
      applyServerSettings,
      error,
      isLoading,
      isSaving,
      refreshServerState,
      serverState,
      setDataSourceSettings,
    ],
  );

  return (
    <ServerStateContext.Provider value={contextValue}>
      {children}
    </ServerStateContext.Provider>
  );
}

export function useServerState() {
  const serverState = useContext(ServerStateContext);

  if (!serverState) {
    throw new Error("useServerState must be used within ServerStateProvider");
  }

  return serverState;
}

export function useServerSettings() {
  const state = useServerState();
  return state.serverState.settings;
}

export function useGRPCServerSettings() {
  const settings = useServerSettings();
  return settings.grpc;
}

function formatError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

type RustServerSettings = {
  general: ServerSettings["general"];
  http: TransportSettings;
  websocket: TransportSettings;
  grpc: TransportSettings;
};

type RustServerState = {
  settings: RustServerSettings;
  status: ServerState["status"];
};

function mergeRustServerState(
  state: RustServerState,
  dataSource: ServerDataSourceSettings,
): ServerState {
  return {
    ...state,
    settings: {
      ...state.settings,
      dataSource,
    },
  };
}

function toRustServerSettings(settings: ServerSettings): RustServerSettings {
  return {
    general: settings.general,
    http: settings.http,
    websocket: settings.websocket,
    grpc: settings.grpc,
  };
}
