import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@tauri-apps/api/core";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Hud } from "./Hud";
import { Curtain } from "./Curtain";
import { Dashboard } from "./Dashboard";
import "./styles.css";

const label = isTauri() ? getCurrentWindow().label : "main";
document.documentElement.dataset.window = label;

const View = label === "main" ? Dashboard : label === "curtain" ? Curtain : label === "hud" ? Hud : null;
if (View) {
  createRoot(document.getElementById("root")!).render(<StrictMode><View /></StrictMode>);
}
