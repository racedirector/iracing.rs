import { PropsWithChildren, useMemo, useRef } from "react";
import { HashRouter, useLocation, useNavigate } from "react-router";
import { Header } from "./components/Header";
import {
  IRacingConnectionStateProvider,
  useIRacingConnectionState,
} from "./contexts/IRacingConnectionStateContext";
import { AppNavigation } from "./navigation";
import "./App.css";
import { BroadcastClientProvider } from "./contexts/BroadcastClient";
import {
  ServerStateProvider,
  useGRPCServerSettings,
} from "./contexts/ServerStateContext";
import { BroadcastClient } from "./constants/broadcast-client";

function GRPCBroadcastProvider({ children }: PropsWithChildren<unknown>) {
  const serverSettings = useGRPCServerSettings();
  const connectionUrl = useMemo(() => {
    return new URL(`http://${serverSettings.host}:${serverSettings.port}`);
  }, [serverSettings]);

  const client = useRef(new BroadcastClient(connectionUrl.toString()));

  return (
    <BroadcastClientProvider client={client.current}>
      {children}
    </BroadcastClientProvider>
  );
}

function Providers({ children }: PropsWithChildren<unknown>) {
  return (
    <IRacingConnectionStateProvider>
      <ServerStateProvider>
        <GRPCBroadcastProvider>
          <HashRouter>{children}</HashRouter>
        </GRPCBroadcastProvider>
      </ServerStateProvider>
    </IRacingConnectionStateProvider>
  );
}

function InnerApp() {
  const state = useIRacingConnectionState();
  const navigate = useNavigate();
  const location = useLocation();

  const isSettingsRoute = useMemo(
    () => location.pathname === "/settings",
    [location.pathname],
  );

  const headerItems = useMemo(() => {
    return [
      {
        label: "iRacing Process",
        status: state.process,
      },
      {
        label: "Simulation",
        status: state.sim,
      },
      {
        label: "Telemetry",
        status: state.telemetry,
      },
    ];
  }, [state]);

  const connectionStatus = useMemo(() => {
    const statuses = Object.values(state);

    if (statuses.every((status) => status === "connected")) {
      return "connected";
    }

    if (statuses.every((status) => status === "disconnected")) {
      return "disconnected";
    }

    return "checking";
  }, [state]);

  return (
    <div className="app-shell">
      <Header
        connectionStatus={connectionStatus}
        items={headerItems}
        isEnabled={isSettingsRoute}
        onToggleSettings={(isEnabled) => {
          navigate(isEnabled ? "/settings" : "/broadcast");
        }}
      />

      <main className="app-content" aria-label="iRacing status workspace">
        <AppNavigation />
      </main>
    </div>
  );
}

export function App() {
  return (
    <Providers>
      <InnerApp />
    </Providers>
  );
}

export default App;
