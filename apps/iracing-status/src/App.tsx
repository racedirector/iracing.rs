import { HashRouter } from "react-router";
import { Header } from "./components/Header";
import { useIRacingConnectionState } from "./contexts/IRacingConnectionStateContext";
import { AppNavigation } from "./navigation";
import "./App.css";

function App() {
  const connectionState = useIRacingConnectionState();

  return (
    <HashRouter>
      <div className="app-shell">
        <Header connectionState={connectionState} />

        <main className="app-content" aria-label="iRacing status workspace">
          <AppNavigation />
        </main>
      </div>
    </HashRouter>
  );
}

export default App;
