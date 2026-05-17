import {
  HashRouter,
  Navigate,
  NavLink,
  Route,
  Routes,
  useLocation,
  useMatch,
} from "react-router";
import { IbtScreen } from "../screens/IbtScreen";
import { LiveScreen } from "../screens/LiveScreen";
import { ServerScreen } from "../screens/ServerScreen";

type RootTab = {
  label: string;
  path: string;
  panelId: string;
};

const tabs: RootTab[] = [
  { label: "Live", path: "/live", panelId: "live-panel" },
  { label: "IBT", path: "/ibt", panelId: "ibt-panel" },
  { label: "Server", path: "/server", panelId: "server-panel" },
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
  const activeTab =
    tabs.find((tab) => location.pathname === tab.path) ?? tabs[0];

  return (
    <>
      <nav className="tab-bar" role="tablist" aria-label="Status views">
        {tabs.map((tab) => (
          <AppNavigationTab key={tab.path} tab={tab} />
        ))}
      </nav>

      <div
        className="tab-panel"
        id={activeTab.panelId}
        role="tabpanel"
        aria-labelledby={`${activeTab.panelId}-tab`}
      >
        <Routes>
          <Route index element={<Navigate replace to="/live" />} />
          <Route path="/live" element={<LiveScreen />} />
          <Route path="/ibt" element={<IbtScreen />} />
          <Route path="/server" element={<ServerScreen />} />
          <Route path="*" element={<Navigate replace to="/live" />} />
        </Routes>
      </div>
    </>
  );
}

export function AppNavigation() {
  return (
    <HashRouter>
      <RoutedNavigation />
    </HashRouter>
  );
}
