import React from "react";
import ReactDOM from "react-dom/client";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import "./styles.css";

const desktop = isTauri();
const windowLabel = desktop ? getCurrentWindow().label : "browser";
document.documentElement.dataset.window = windowLabel;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App desktop={desktop} windowLabel={windowLabel} />
  </React.StrictMode>,
);
