import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@tauri-apps/api/core";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Dashboard } from "./Dashboard";
import "./styles.css";

const label = isTauri() ? getCurrentWindow().label : "main";
document.documentElement.dataset.window = label;

if (label === "main") {
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <Dashboard />
    </StrictMode>,
  );
}
