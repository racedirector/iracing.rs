import { createContext, useContext, useMemo, type ReactNode } from "react";
import { BroadcastClientAPI } from "./types";

interface BroadcastClientContextValue {
  client: BroadcastClientAPI;
}

const BroadcastClientContext =
  createContext<BroadcastClientContextValue | null>(null);

interface BroadcastClientProviderProps<T extends BroadcastClientAPI> {
  client: T;
  children: ReactNode;
}

export function BroadcastClientProvider<T extends BroadcastClientAPI>({
  client,
  children,
}: BroadcastClientProviderProps<T>) {
  const contextValue = useMemo<BroadcastClientContextValue>(
    () => ({
      client,
    }),
    [],
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

  return broadcastClient.client;
}
