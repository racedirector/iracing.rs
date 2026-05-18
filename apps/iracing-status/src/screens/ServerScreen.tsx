import { useEffect, useState } from "react";
import { useServerState } from "../contexts/ServerStateContext";
import {
  defaultServerState,
  formatTransportStatus,
  getTransportEnabled,
  setTransportEnabled,
  type ServerSettings,
  type ServerState,
  type TransportKey,
  type TransportRuntimeStatus,
  type TransportSettings,
} from "../server";

type ServerSection = "general" | TransportKey;

type ServerNavigationItem = {
  key: ServerSection;
  label: string;
  disabled: boolean;
};

const transportLabels: Record<TransportKey, string> = {
  http: "HTTP",
  websocket: "WebSocket",
  grpc: "gRPC",
};

const transportDescriptions: Record<TransportKey, string> = {
  http: "Serves local status and health responses over HTTP.",
  websocket: "Accepts local WebSocket clients for future live status streams.",
  grpc: "Serves iRacing broadcast controls over tonic gRPC.",
};

export function ServerScreen() {
  const { applyServerSettings, error, isLoading, isSaving, serverState } =
    useServerState();
  const [activeSection, setActiveSection] = useState<ServerSection>("general");
  const [draftSettings, setDraftSettings] = useState<ServerSettings>(
    defaultServerState.settings,
  );

  useEffect(() => {
    setDraftSettings(serverState.settings);
  }, [serverState.settings]);

  async function applySettings(
    settings: ServerSettings,
    fallbackSection: ServerSection = activeSection,
  ) {
    try {
      const nextState = await applyServerSettings(settings);
      setDraftSettings(nextState.settings);

      if (
        fallbackSection !== "general" &&
        !getTransportEnabled(nextState.settings, fallbackSection)
      ) {
        setActiveSection("general");
      }
    } catch {
      setDraftSettings(serverState.settings);
    }
  }

  function handleTransportToggle(transport: TransportKey, enabled: boolean) {
    const nextSettings = setTransportEnabled(draftSettings, transport, enabled);
    setDraftSettings(nextSettings);
    applySettings(nextSettings, transport);
  }

  function handleTransportSettingsChange(
    transport: TransportKey,
    settings: TransportSettings,
  ) {
    setDraftSettings((currentSettings) => ({
      ...currentSettings,
      [transport]: settings,
    }));
  }

  function handleTransportSettingsApply(transport: TransportKey) {
    applySettings(draftSettings, transport);
  }

  const navigationItems = getNavigationItems(draftSettings);

  return (
    <section className="server-screen" aria-labelledby="server-screen-title">
      <div className="server-screen__header">
        <div>
          <h2 id="server-screen-title">Server</h2>
          <p>Manage local transports exposed by this app process.</p>
        </div>
        <ServerStatusSummary serverState={serverState} />
      </div>

      {error ? (
        <p className="server-screen__error" role="alert">
          {error}
        </p>
      ) : null}

      <div className="server-layout">
        <nav className="server-sidebar" aria-label="Server settings sections">
          {navigationItems.map((item) => (
            <button
              aria-current={activeSection === item.key ? "page" : undefined}
              className={
                activeSection === item.key
                  ? "server-sidebar__item server-sidebar__item--active"
                  : "server-sidebar__item"
              }
              disabled={item.disabled}
              key={item.key}
              onClick={() => setActiveSection(item.key)}
              type="button"
            >
              {item.label}
            </button>
          ))}
        </nav>

        <div className="server-panel" aria-live="polite">
          {isLoading ? (
            <p className="server-panel__muted">Loading server settings...</p>
          ) : null}

          {!isLoading && activeSection === "general" ? (
            <GeneralSettingsPanel
              isSaving={isSaving}
              settings={draftSettings}
              onToggle={handleTransportToggle}
            />
          ) : null}

          {!isLoading && activeSection !== "general" ? (
            <TransportSettingsPanel
              description={transportDescriptions[activeSection]}
              isSaving={isSaving}
              settings={draftSettings[activeSection]}
              status={serverState.status[activeSection]}
              title={transportLabels[activeSection]}
              onApply={() => handleTransportSettingsApply(activeSection)}
              onChange={(settings) =>
                handleTransportSettingsChange(activeSection, settings)
              }
            />
          ) : null}
        </div>
      </div>
    </section>
  );
}

function GeneralSettingsPanel({
  isSaving,
  settings,
  onToggle,
}: {
  isSaving: boolean;
  settings: ServerSettings;
  onToggle: (transport: TransportKey, enabled: boolean) => void;
}) {
  return (
    <div className="server-panel__content">
      <div className="server-panel__heading">
        <h3>Transports</h3>
        <p>Enable only the transports that should bind local ports.</p>
      </div>

      <div className="server-toggle-list">
        {(["http", "websocket", "grpc"] as TransportKey[]).map((transport) => (
          <label className="server-toggle" key={transport}>
            <span>
              <strong>{transportLabels[transport]}</strong>
              <small>{transportDescriptions[transport]}</small>
            </span>
            <input
              checked={getTransportEnabled(settings, transport)}
              disabled={isSaving}
              onChange={(event) =>
                onToggle(transport, event.currentTarget.checked)
              }
              type="checkbox"
            />
          </label>
        ))}
      </div>
    </div>
  );
}

function TransportSettingsPanel({
  description,
  isSaving,
  settings,
  status,
  title,
  onApply,
  onChange,
}: {
  description: string;
  isSaving: boolean;
  settings: TransportSettings;
  status: TransportRuntimeStatus;
  title: string;
  onApply: () => void;
  onChange: (settings: TransportSettings) => void;
}) {
  return (
    <form
      className="server-panel__content"
      onSubmit={(event) => {
        event.preventDefault();
        onApply();
      }}
    >
      <div className="server-panel__heading">
        <h3>{title}</h3>
        <p>{description}</p>
      </div>

      <div className={`server-runtime server-runtime--${status.kind}`}>
        <span>{formatTransportStatus(status)}</span>
      </div>

      <div className="server-field-grid">
        <label className="server-field">
          <span>Host</span>
          <input
            autoComplete="off"
            onChange={(event) =>
              onChange({ ...settings, host: event.currentTarget.value })
            }
            required
            type="text"
            value={settings.host}
          />
        </label>

        <label className="server-field">
          <span>Port</span>
          <input
            max={65535}
            min={1}
            onChange={(event) =>
              onChange({
                ...settings,
                port: Number(event.currentTarget.value),
              })
            }
            required
            type="number"
            value={settings.port}
          />
        </label>
      </div>

      <button className="server-apply-button" disabled={isSaving} type="submit">
        Apply
      </button>
    </form>
  );
}

function ServerStatusSummary({ serverState }: { serverState: ServerState }) {
  const runningCount = [
    serverState.status.http,
    serverState.status.websocket,
    serverState.status.grpc,
  ].filter((status) => status.kind === "running").length;

  return (
    <div className="server-summary" aria-label="Running transports">
      <strong>{runningCount}</strong>
      <span>running</span>
    </div>
  );
}

function getNavigationItems(settings: ServerSettings): ServerNavigationItem[] {
  return [
    { key: "general", label: "General", disabled: false },
    {
      key: "http",
      label: "HTTP",
      disabled: !settings.general.httpEnabled,
    },
    {
      key: "websocket",
      label: "WebSocket",
      disabled: !settings.general.websocketEnabled,
    },
    {
      key: "grpc",
      label: "gRPC",
      disabled: !settings.general.grpcEnabled,
    },
  ];
}
