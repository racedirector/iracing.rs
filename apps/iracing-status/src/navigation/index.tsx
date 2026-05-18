import {
  Navigate,
  NavLink,
  Route,
  Routes,
  useLocation,
  useMatch,
} from "react-router";
import { BroadcastClientScreen } from "../screens/BroadcastClientScreen";
import { ServerSettingsScreen } from "../screens/ServerSettingsScreen";

type RootTab = {
  label: string;
  path: string;
  panelId: string;
};

const tabs: RootTab[] = [
  {
    label: "Broadcast",
    path: "/broadcast",
    panelId: "broadcast-panel",
  },
];

function AppNavigationTab({ tab }: { tab: RootTab }) {
  const isActive = Boolean(useMatch({ path: tab.path, end: true }));

  return (
    <NavLink
      aria-controls={tab.panelId}
      aria-selected={isActive}
      className={isActive ? "tab-button tab-button--active" : "tab-button"}
      id={`${tab.panelId}-tab`}
      role="tab"
      to={tab.path}
    >
      {tab.label}
    </NavLink>
  );
}

function RoutedNavigation() {
  const location = useLocation();
  const activeTab = tabs.find((tab) => location.pathname === tab.path);
  const isSettingsRoute = location.pathname === "/settings";

  return (
    <>
      {!isSettingsRoute ? (
        <nav className="tab-bar" role="tablist" aria-label="Status views">
          {tabs.map((tab) => (
            <AppNavigationTab key={tab.path} tab={tab} />
          ))}
        </nav>
      ) : null}

      <div
        className={isSettingsRoute ? "tab-panel tab-panel--full" : "tab-panel"}
        id={activeTab?.panelId}
        role={activeTab ? "tabpanel" : undefined}
        aria-labelledby={activeTab ? `${activeTab.panelId}-tab` : undefined}
      >
        <Routes>
          <Route index element={<Navigate replace to="/broadcast" />} />
          <Route path="/broadcast" element={<BroadcastClientScreen />} />
          <Route path="/settings" element={<ServerSettingsScreen />} />
          <Route path="*" element={<Navigate replace to="/broadcast" />} />
        </Routes>
      </div>
    </>
  );
}

export function AppNavigation() {
  return <RoutedNavigation />;
}
