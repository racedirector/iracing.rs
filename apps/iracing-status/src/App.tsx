import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type ConnectionStatus = "disconnected" | "checking" | "connected";

type IRacingConnectionState = {
  process: ConnectionStatus;
  sim: ConnectionStatus;
  telemetry: ConnectionStatus;
};

type StatusIndicatorProps = {
  label: string;
  status: ConnectionStatus;
};

type StatusLightProps = {
  status: ConnectionStatus;
};

const CONNECTION_STATE_CHANGED_EVENT = "iracing://connection-state-changed";

const initialConnectionState: IRacingConnectionState = {
  process: "checking",
  sim: "disconnected",
  telemetry: "disconnected",
};

const disconnectedConnectionState: IRacingConnectionState = {
  process: "disconnected",
  sim: "disconnected",
  telemetry: "disconnected",
};

function getConnectionStatus(
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

function formatStatus(status: ConnectionStatus) {
  return status[0].toUpperCase() + status.slice(1);
}

function StatusLight({ status }: StatusLightProps) {
  return <span className={`status-light status-light--${status}`} />;
}

function StatusDetail({ label, status }: StatusIndicatorProps) {
  return (
    <div className="status-detail">
      <StatusLight status={status} />
      <div className="status-copy">
        <span className="status-label">{label}</span>
        <span className="status-value">{formatStatus(status)}</span>
      </div>
    </div>
  );
}

function App() {
  const [connectionState, setConnectionState] = useState<IRacingConnectionState>(
    initialConnectionState,
  );

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
          setConnectionState(disconnectedConnectionState);
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

  const lifecycleStatus: StatusIndicatorProps[] = [
    { label: "iRacing Process", status: connectionState.process },
    { label: "Sim Status", status: connectionState.sim },
    { label: "Live Telemetry", status: connectionState.telemetry },
  ];
  const aggregateStatus = getConnectionStatus(connectionState);

  return (
    <div className="app-shell">
      <header className="app-nav">
        <h1 className="app-title">iRacing Status</h1>

        <div className="connection-summary">
          <button
            className="connection-trigger"
            type="button"
            aria-describedby="connection-popover"
            aria-label={`iRacing connection ${formatStatus(aggregateStatus)}`}
          >
            <StatusLight status={aggregateStatus} />
            <span>iRacing connection</span>
            <span className="status-value">{formatStatus(aggregateStatus)}</span>
          </button>

          <div
            className="connection-popover"
            id="connection-popover"
            role="status"
            aria-live="polite"
            aria-label="iRacing connection details"
          >
            {lifecycleStatus.map((item) => (
              <StatusDetail
                key={item.label}
                label={item.label}
                status={item.status}
              />
            ))}
          </div>
        </div>
      </header>

      <main
        className="app-content"
        aria-label="iRacing status workspace"
      />
    </div>
  );
}

export default App;
