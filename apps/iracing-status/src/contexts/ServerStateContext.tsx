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
  type ServerSettings,
  type ServerState,
} from "../server";

type ServerStateContextValue = {
  applyServerSettings: (settings: ServerSettings) => Promise<ServerState>;
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
      const nextState = await invoke<ServerState>("get_server_state");
      setServerState(nextState);
      setError(null);
      return nextState;
    } catch (refreshError) {
      const message = formatError(refreshError);
      setError(message);
      throw new Error(message);
    }
  }, []);

  const applyServerSettings = useCallback(async (settings: ServerSettings) => {
    setIsSaving(true);
    setError(null);

    try {
      const nextState = await invoke<ServerState>("set_server_settings", {
        settings,
      });
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

  useEffect(() => {
    let isMounted = true;

    async function loadServerState() {
      try {
        const nextState = await invoke<ServerState>("get_server_state");
        if (isMounted) {
          setServerState(nextState);
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
    }),
    [
      applyServerSettings,
      error,
      isLoading,
      isSaving,
      refreshServerState,
      serverState,
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

function formatError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}
