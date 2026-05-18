import { useLocation } from "react-router";
import { ConnectionStatus, ConnectionStatusProps } from "./ConnectionStatus";
import { useEffect } from "react";

interface HeaderProps extends ConnectionStatusProps {
  isEnabled: boolean;
  onToggleSettings: (isEnabled: boolean) => void;
}

export function Header({
  connectionStatus,
  items,
  isEnabled,
  onToggleSettings,
}: HeaderProps) {
  const location = useLocation();
  const isSettingsRoute = location.pathname === "/settings";

  useEffect(() => {
    onToggleSettings(isEnabled);
  }, [isEnabled, onToggleSettings]);

  return (
    <header className="app-nav">
      <h1 className="app-title">iRacing Telemetry</h1>
      <div className="app-nav__actions">
        <button
          aria-label={isEnabled ? "Close settings" : "Open settings"}
          aria-pressed={isSettingsRoute}
          className={
            isEnabled
              ? "app-settings-button app-settings-button--active"
              : "app-settings-button"
          }
          onClick={() => onToggleSettings(!isEnabled)}
          type="button"
        >
          <span aria-hidden="true">&#9881;</span>
        </button>
        <ConnectionStatus connectionStatus={connectionStatus} items={items} />
      </div>
    </header>
  );
}
