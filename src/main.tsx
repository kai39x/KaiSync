import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { inTauri } from "./api";
import "./i18n";
import { StoreProvider } from "./state/store";
import "./styles/global.css";
import { TrayMenu } from "./tray/TrayMenu";

// The same bundle drives the main window and the small glass tray-menu window.
const isTrayMenu = inTauri ? getCurrentWindow().label === "tray-menu" : new URLSearchParams(location.search).get("view") === "tray";
if (isTrayMenu) {
  document.documentElement.style.background = "transparent";
  document.body.style.background = "transparent";
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isTrayMenu ? (
      <StoreProvider>
        <TrayMenu />
      </StoreProvider>
    ) : (
      <App />
    )}
  </React.StrictMode>,
);
