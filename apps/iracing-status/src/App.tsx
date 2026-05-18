import { useMemo } from "react";
import { HashRouter, useLocation, useNavigate } from "react-router";
import { Header } from "./components/Header";
import { useIRacingConnectionState } from "./contexts/IRacingConnectionStateContext";
import { AppNavigation } from "./navigation";
import "./App.css";

function App() {
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
    <HashRouter>
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
    </HashRouter>
  );
}

export default App;
