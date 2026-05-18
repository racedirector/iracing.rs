import { useLocation, useNavigate } from "react-router";
import type { IRacingConnectionState } from "../connection";
import { ConnectionStatus } from "./ConnectionStatus";

type HeaderProps = {
  connectionState: IRacingConnectionState;
};

export function Header({ connectionState }: HeaderProps) {
  const location = useLocation();
  const navigate = useNavigate();
  const isSettingsRoute = location.pathname === "/settings";

  function handleSettingsToggle() {
    navigate(isSettingsRoute ? "/broadcast" : "/settings");
  }

  return (
    <header className="app-nav">
      <h1 className="app-title">iRacing Telemetry</h1>
      <div className="app-nav__actions">
        <button
          aria-label={isSettingsRoute ? "Close settings" : "Open settings"}
          aria-pressed={isSettingsRoute}
          className={
            isSettingsRoute
              ? "app-settings-button app-settings-button--active"
              : "app-settings-button"
          }
          onClick={handleSettingsToggle}
          type="button"
        >
          <span aria-hidden="true">&#9881;</span>
        </button>
        <ConnectionStatus connectionState={connectionState} />
      </div>
    </header>
  );
}
