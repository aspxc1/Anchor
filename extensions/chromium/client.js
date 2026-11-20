export const DEFAULT_PORT = 17321;

export class BridgeError extends Error {
  constructor(kind, status) {
    super(kind === "invalid" ? "The pairing token was rejected." : "Anchor is offline.");
    this.kind = kind;
    this.status = status;
  }
}

export async function readConfig() {
  const { port, token } = await chrome.storage.local.get({ port: DEFAULT_PORT, token: "" });
  return {
    port: Number.isInteger(port) && port >= 1024 && port <= 65535 ? port : DEFAULT_PORT,
    token: typeof token === "string" && /^[0-9a-f]{32}$/.test(token) ? token : "",
  };
}

export async function request(config, path, options = {}) {
  if (!config.token) throw new BridgeError("invalid");
  let response;
  try {
    response = await fetch(`http://127.0.0.1:${config.port}/api/v1/${path}`, {
      method: options.method ?? "GET",
      body: options.body,
      headers: {
        "X-Anchor-Token": config.token,
        ...(options.body === undefined ? {} : { "Content-Type": "application/json" }),
      },
      credentials: "omit",
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(300),
    });
    if (response.status === 401) throw new BridgeError("invalid", 401);
    if (!response.ok) throw new BridgeError(response.status === 409 ? "conflict" : "offline", response.status);
    return await response.json();
  } catch (error) {
    if (error instanceof BridgeError) throw error;
    throw new BridgeError("offline");
  }
}

export function getStatus(config) {
  return request(config, "status");
}

export function openSocket(config) {
  const token = new URLSearchParams({ token: config.token });
  return new WebSocket(`ws://127.0.0.1:${config.port}/api/v1/ws?${token}`);
}

export function targetUrl(value) {
  try {
    const url = new URL(value);
    if (!["http:", "https:"].includes(url.protocol)) return null;
    url.username = "";
    url.password = "";
    return url;
  } catch {
    return null;
  }
}

export function formatTime(seconds) {
  if (!Number.isFinite(seconds) || seconds < 0) return "00:00";
  return [Math.floor(seconds / 60), Math.floor(seconds % 60)]
    .map((part) => String(part).padStart(2, "0")).join(":");
}

export function stateLabel(state) {
  if (state === "ActiveFocus") return "Focus";
  if (state === "HallPass") return "Hall Pass";
  return state ?? "Idle";
}
