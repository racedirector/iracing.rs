import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { BroadcastClientProvider } from "./contexts/BroadcastClientContext";
import { IRacingConnectionStateProvider } from "./contexts/IRacingConnectionStateContext";
import { ServerStateProvider } from "./contexts/ServerStateContext";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <IRacingConnectionStateProvider>
      <ServerStateProvider>
        <BroadcastClientProvider>
          <App />
        </BroadcastClientProvider>
      </ServerStateProvider>
    </IRacingConnectionStateProvider>
  </React.StrictMode>,
);
