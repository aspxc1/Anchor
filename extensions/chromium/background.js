import { readConfig, getStatus, request, openSocket, targetUrl } from './client.js';
import { isAllowed, normalizeDomain } from './domains.js';

const CACHE_MS = 1500;
const RECHECK_ALARM = 'anchor-recheck';
let config;
let generation = 0;
let revision = 0;
let policyRevision = 0;
let cached = null;
let socket = null;
let retryTimer = null;
let pingTimer = null;
let retryDelay = 1000;
const navigation = new Map();
const lastSafe = new Map();
const recentBlocks = new Map();

function validStatus(value) {
  return value && ['Idle', 'ActiveFocus', 'HallPass', 'Break'].includes(value.state)
    && Array.isArray(value.allowed_domains);
}
function clearStatus() {
  cached = null;
  revision++;
  policyRevision++;
}
function policyOf(status) {
  return JSON.stringify([status.state, status.session_id, status.allowed_domains, status.allow_localhost_domains]);
}
function setStatus(status) {
  if (!validStatus(status)) return false;
  const changed = !cached || policyOf(cached.status) !== policyOf(status);
  cached = { status, at: Date.now() };
  revision++;
  if (changed) policyRevision++;
  return changed;
}
function connected() { return socket?.readyState === WebSocket.OPEN; }

async function statusNow(force = false) {
  if (!config) return null;
  if (!force && cached && Date.now() - cached.at < CACHE_MS) return cached.status;
  const currentGeneration = generation;
  const currentRevision = revision;
  try {
    const status = await getStatus(config);
    if (currentGeneration !== generation) return null;
    // A socket event or a close after the request started is newer evidence.
    if (currentRevision !== revision) return cached?.status ?? null;
    if (!validStatus(status)) { clearStatus(); return null; }
    setStatus(status);
    return status;
  } catch {
    if (currentGeneration === generation && currentRevision === revision) clearStatus();
    return null;
  }
}

function connect() {
  if (!config?.token || socket || retryTimer) return;
  const ownGeneration = generation;
  let ws;
  try { ws = openSocket(config); } catch { scheduleRetry(ownGeneration); return; }
  socket = ws;
  ws.addEventListener('open', () => {
    if (ownGeneration !== generation || socket !== ws) return;
    retryDelay = 1000;
    clearInterval(pingTimer);
    pingTimer = setInterval(() => {
      if (socket === ws && ws.readyState === WebSocket.OPEN) ws.send('ping');
    }, 20000);
  });
  ws.addEventListener('message', (event) => {
    if (ownGeneration !== generation || socket !== ws) return;
    try {
      if (setStatus(JSON.parse(event.data))) void sweepActive();
    } catch { /* Ignore non-status messages. */ }
  });
  ws.addEventListener('close', () => {
    if (ownGeneration !== generation || socket !== ws) return;
    socket = null;
    clearInterval(pingTimer);
    pingTimer = null;
    clearStatus();
    scheduleRetry(ownGeneration);
  });
  ws.addEventListener('error', () => { if (socket === ws) ws.close(); });
}
function scheduleRetry(ownGeneration) {
  if (ownGeneration !== generation || retryTimer) return;
  retryTimer = setTimeout(() => {
    retryTimer = null;
    if (ownGeneration === generation) connect();
  }, retryDelay);
  retryDelay = Math.min(retryDelay * 2, 30000);
}
async function configure() {
  const ownGeneration = ++generation;
  clearStatus();
  clearTimeout(retryTimer); retryTimer = null;
  clearInterval(pingTimer); pingTimer = null;
  const previous = socket; socket = null;
  if (previous) previous.close();
  config = null;
  retryDelay = 1000;
  const next = await readConfig();
  if (ownGeneration !== generation) return;
  config = next;
  connect();
  await sweepActive();
}

async function inspect(tabId, value) {
  const target = targetUrl(value);
  if (!target || tabId == null || tabId < 0) return;
  const sequence = (navigation.get(tabId) || 0) + 1;
  navigation.set(tabId, sequence);
  let status = await statusNow();
  if (navigation.get(tabId) !== sequence || !status) return;
  if (status.state !== 'ActiveFocus' || isAllowed(target.href, status)) {
    // Save only an allowed URL, never the blocked page or credentials.
    if (isAllowed(target.href, status)) lastSafe.set(tabId, target.href);
    return;
  }
  // A disconnected socket cannot prove that a cached focus policy is current.
  if (!connected()) {
    status = await statusNow(true);
    if (!status || navigation.get(tabId) !== sequence || status.state !== 'ActiveFocus' || isAllowed(target.href, status)) return;
  }
  const inspectedPolicy = policyRevision;
  const policyGeneration = generation;
  let tab;
  try { tab = await chrome.tabs.get(tabId); } catch { return; }
  if (navigation.get(tabId) !== sequence || policyRevision !== inspectedPolicy || generation !== policyGeneration) return;
  // Verify that the event still describes the pending or current navigation.
  if (tab.pendingUrl !== value && tab.url !== value) return;
  const domain = normalizeDomain(target.hostname);
  const key = `${status.session_id}:${domain}`;
  const params = new URLSearchParams({ target: target.href, domain });
  const safe = lastSafe.get(tabId);
  if (safe && isAllowed(safe, status)) params.set('return', safe);
  const blockUrl = chrome.runtime.getURL('blocked.html') + '?' + params;
  try {
    await chrome.tabs.update(tabId, { url: blockUrl });
    const now = Date.now();
    for (const [blocked, at] of recentBlocks) {
      if (now - at >= 3000) recentBlocks.delete(blocked);
    }
    if (!recentBlocks.has(key)) {
      recentBlocks.set(key, now);
      void request(config, 'violation', {
        method: 'POST',
        body: JSON.stringify({ domain, url: target.href, tab_title: tab.url === value ? tab.title || '' : '', session_id: status.session_id ?? null }),
      }).catch(() => {});
    }
  } catch { /* A closed tab or navigation race should not prevent browsing. */ }
}
async function sweepActive() {
  try {
    const tabs = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    for (const tab of tabs) void inspect(tab.id, tab.pendingUrl || tab.url);
  } catch { /* Browser may be shutting down. */ }
}

chrome.webNavigation.onBeforeNavigate.addListener((details) => {
  if (details.frameId === 0) void inspect(details.tabId, details.url);
});
chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.url) void inspect(tabId, changeInfo.url);
});
chrome.tabs.onActivated.addListener(({ tabId }) => {
  void chrome.tabs.get(tabId).then((tab) => inspect(tabId, tab.pendingUrl || tab.url)).catch(() => {});
});
chrome.tabs.onRemoved.addListener((tabId) => {
  navigation.delete(tabId); lastSafe.delete(tabId);
});
chrome.windows.onFocusChanged.addListener(() => { void sweepActive(); });
chrome.storage.onChanged.addListener((changes, area) => {
  if (area === 'local' && (changes.port || changes.token)) void configure();
});
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === RECHECK_ALARM) { connect(); void sweepActive(); }
});
chrome.runtime.onMessage.addListener((message, sender, reply) => {
  if (sender.id !== chrome.runtime.id || message?.type !== 'refresh-anchor') return;
  void statusNow(true).then(() => reply({ ok: true }));
  return true;
});
void chrome.alarms.create(RECHECK_ALARM, { periodInMinutes: 0.5 });
void configure();
