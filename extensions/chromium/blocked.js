import { formatTime, getStatus, readConfig, request, targetUrl } from "./client.js";
import { isAllowed } from "./domains.js";

const params = new URLSearchParams(location.search);
const target = targetUrl(params.get("target"));
const previous = targetUrl(params.get("return"));
const feedback = document.getElementById("feedback");
const pass = document.getElementById("pass");
const countdown = document.getElementById("countdown");
const timer = document.getElementById("timer");
const domain = document.getElementById("domain");
let checking = false;
let usingPass = false;
let navigating = false;

function leave(url) {
  if (navigating) return;
  navigating = true;
  location.replace(url);
}

if (target) {
  const host = target.hostname.replace(/^www\./i, "");
  domain.textContent = host;
  document.title = `${host} · Saved for later · Anchor`;
} else {
  feedback.textContent = "This blocked page has no destination.";
}

document.getElementById("back").addEventListener("click", () => {
  if (previous && previous.href !== target?.href) {
    leave(previous.href);
    return;
  }
  void chrome.runtime.sendMessage({ type: "leave-blocked-tab" }).catch(() => {});
});

async function refresh() {
  if (!target || checking || usingPass || navigating) return;
  checking = true;
  try {
    const status = await getStatus(await readConfig());
    if (isAllowed(target.href, status)) {
      leave(target.href);
      return;
    }
    document.getElementById("goal").textContent = status.goal;
    timer.textContent = formatTime(status.remaining_seconds);
    countdown.hidden = false;
    pass.textContent = `Use 60s Hall Pass (${status.hall_passes_remaining} left)`;
    pass.disabled = status.hall_passes_remaining < 1;
  } catch {
    leave(target.href);
  } finally {
    checking = false;
  }
}

pass.addEventListener("click", async () => {
  if (!target || usingPass) return;
  usingPass = true;
  pass.disabled = true;
  feedback.textContent = "";
  try {
    await request(await readConfig(), "hallpass", { method: "POST" });
    await chrome.runtime.sendMessage({ type: "refresh-anchor" }).catch(() => {});
    leave(target.href);
  } catch (error) {
    if (error.kind === "conflict") {
      feedback.textContent = "A Hall Pass is unavailable in this phase.";
    } else if (!error.status || error.status === 401) {
      leave(target.href);
    } else {
      feedback.textContent = "Anchor could not start the Hall Pass. Try again.";
    }
  } finally {
    usingPass = false;
    void refresh();
  }
});

document.addEventListener("visibilitychange", () => {
  if (!document.hidden) void refresh();
});

void refresh();
setInterval(refresh, 1000);
