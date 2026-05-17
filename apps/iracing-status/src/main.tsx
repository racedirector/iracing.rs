import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { IRacingConnectionStateProvider } from "./contexts/IRacingConnectionStateContext";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <IRacingConnectionStateProvider>
      <App />
    </IRacingConnectionStateProvider>
  </React.StrictMode>,
);
