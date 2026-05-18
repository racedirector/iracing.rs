import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const CONNECTION_STATE_CHANGED_EVENT =
  "iracing://connection-state-changed";
export type ConnectionStatus = "disconnected" | "checking" | "connected";

export type IRacingConnectionState = {
  process: ConnectionStatus;
  sim: ConnectionStatus;
  telemetry: ConnectionStatus;
};

const IRacingConnectionStateContext =
  createContext<IRacingConnectionState | null>(null);

interface IRacingConnectionStateProviderProps {
  children: ReactNode;
}

export function IRacingConnectionStateProvider({
  children,
}: IRacingConnectionStateProviderProps) {
  const [connectionState, setConnectionState] =
    useState<IRacingConnectionState>({
      process: "checking",
      sim: "disconnected",
      telemetry: "disconnected",
    });

  useEffect(() => {
    let isMounted = true;
    let shouldUnlisten = false;
    let unlisten: (() => void) | undefined;

    async function observeConnectionState() {
      try {
        unlisten = await listen<IRacingConnectionState>(
          CONNECTION_STATE_CHANGED_EVENT,
          (event) => {
            if (isMounted) {
              setConnectionState(event.payload);
            }
          },
        );

        if (shouldUnlisten) {
          unlisten();
          return;
        }

        const initialState = await invoke<IRacingConnectionState>(
          "observe_connection_state",
        );

        if (isMounted) {
          setConnectionState(initialState);
        }
      } catch {
        if (isMounted) {
          setConnectionState({
            process: "disconnected",
            sim: "disconnected",
            telemetry: "disconnected",
          });
        }
      }
    }

    observeConnectionState();

    return () => {
      isMounted = false;
      shouldUnlisten = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  return (
    <IRacingConnectionStateContext.Provider value={connectionState}>
      {children}
    </IRacingConnectionStateContext.Provider>
  );
}

export function useIRacingConnectionState() {
  const connectionState = useContext(IRacingConnectionStateContext);

  if (!connectionState) {
    throw new Error(
      "useIRacingConnectionState must be used within IRacingConnectionStateProvider",
    );
  }

  return connectionState;
}
