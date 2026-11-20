import { formatTime, getStatus, readConfig, stateLabel } from "./client.js";

const badge = document.getElementById("badge");
const session = document.getElementById("session");
const connection = document.getElementById("connection");
const port = document.getElementById("port");
const token = document.getElementById("token");
const settings = document.getElementById("settings");
const phase = document.getElementById("phase");
const timer = document.getElementById("timer");
const goal = document.getElementById("goal");
let config = await readConfig();
let request = 0;

port.value = String(config.port);
token.value = config.token;
settings.open = !config.token;

async function refresh() {
  const id = ++request;
  try {
    const status = await getStatus(config);
    if (id !== request) return;
    badge.textContent = "Connected";
    badge.dataset.state = "connected";
    session.hidden = status.state === "Idle";
    phase.textContent = stateLabel(status.state);
    timer.textContent = formatTime(status.state === "HallPass" ? status.hall_pass_remaining_seconds : status.remaining_seconds);
    goal.textContent = status.goal;
    connection.hidden = status.state !== "Idle";
    connection.textContent = "Start a session in Anchor to apply its allowed websites.";
  } catch (error) {
    if (id !== request) return;
    session.hidden = true;
    connection.hidden = false;
    const invalid = error.kind === "invalid";
    badge.textContent = invalid ? "Invalid Token" : "Anchor Offline";
    badge.dataset.state = invalid ? "invalid" : "offline";
    connection.textContent = invalid ? "Paste the current pairing token from Anchor’s desktop Settings."
      : "Open Anchor and check the local port in Settings. Browsing stays unrestricted while Anchor is offline.";
  }
}

document.getElementById("pair").addEventListener("submit", async (event) => {
  event.preventDefault();
  try {
    config = { port: Number(port.value), token: token.value.trim().toLowerCase() };
    await chrome.storage.local.set(config);
    void refresh();
  } catch {
    connection.hidden = false;
    connection.textContent = "Could not save the connection. Try again.";
  }
});

void refresh();
setInterval(refresh, 1000);
