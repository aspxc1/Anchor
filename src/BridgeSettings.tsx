import { invoke } from "@tauri-apps/api/core";
import { Clipboard, FolderOpen, Link2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { errorText, send, type Action } from "./session";

interface Settings {
  port: number;
  token: string;
  running: boolean;
  connected: boolean;
  error: string | null;
}

export function BridgeSettings() {
  const [settings, setSettings] = useState<Settings>();
  const [port, setPort] = useState("");
  const [error, setError] = useState<string>();
  const [status, setStatus] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const load = useCallback(async () => invoke<Settings>("bridge_settings"), []);

  useEffect(() => {
    let active = true;
    const read = () => {
      void load().then((result) => { if (active) setSettings(result); }, (reason) => {
        if (active) setError(errorText(reason));
      });
    };
    read();
    const id = setInterval(read, 2500);
    return () => { active = false; clearInterval(id); };
  }, [load]);

  const actualPort = settings?.port;
  useEffect(() => { if (actualPort !== undefined) setPort(String(actualPort)); }, [actualPort]);

  async function update(action: Action, message: string) {
    setBusy(true);
    setError(undefined);
    setStatus(undefined);
    try {
      await send(action);
      setSettings(await load());
      setStatus(message);
      setConfirm(false);
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setBusy(false);
    }
  }

  async function copy() {
    if (!settings) return;
    try {
      await navigator.clipboard.writeText(settings.token);
      setStatus("Pairing token copied.");
      setError(undefined);
    } catch (reason) {
      setError(errorText(reason));
    }
  }

  return (
    <section className="panel desktop-settings browser-settings" aria-labelledby="bridge-heading">
      <div className="settings-intro">
        <div><h2 id="bridge-heading"><Link2 size={17} aria-hidden="true" /> Browser bridge</h2>
          <p>Pair the Anchor extension to enforce allowed websites during focus.</p>
        </div>
        <span className={`connection-label ${settings?.connected ? "connected" : ""}`}>
          {settings?.connected ? "Extension connected" : settings?.running ? "Ready to pair" : "Bridge offline"}
        </span>
      </div>
      <form onSubmit={(event) => {
        event.preventDefault();
        void update({ type: "set_bridge_port", port: Number(port) }, "Bridge port saved.");
      }}>
        <label className="field"><span>Local port</span><input type="number" min={1024} max={65535} value={port} required onChange={(event) => setPort(event.target.value)} /></label>
        <button className="quiet-button" disabled={busy || !settings}>Save port</button>
        <button type="button" className="quiet-button" disabled={!settings} onClick={() => void copy()}><Clipboard size={15} /> Copy Pairing Token</button>
        <button type="button" className="text-button" disabled={busy || !settings} onClick={() => setConfirm((value) => !value)}>Regenerate token</button>
      </form>
      {confirm && <div className="settings-confirm">
        <p>Regenerating the token disconnects paired extensions.</p>
        <button className="quiet-button danger-button" disabled={busy} onClick={() => void update({ type: "rotate_bridge_token" }, "New pairing token ready.")}>Regenerate</button>
      </div>}
      {status && <p className="session-notice" role="status">{status}</p>}
      {(error || settings?.error) && <p className="action-error" role="alert">{error || settings?.error}</p>}
      <details>
        <summary>Load and pair the extension</summary>
        <ol>
          <li>Open <code>chrome://extensions</code>, <code>edge://extensions</code>, or <code>brave://extensions</code>.</li>
          <li>Turn on Developer mode, choose <strong>Load unpacked</strong>, and select the <code>extensions/chromium</code> folder.</li>
          <li>Open the Anchor extension and paste your pairing token and local port into its settings.</li>
        </ol>
        <button type="button" className="quiet-button extension-folder" onClick={() => {
          void invoke("open_extension_folder").catch((reason) => setError(errorText(reason)));
        }}><FolderOpen size={15} /> Open extension folder</button>
      </details>
    </section>
  );
}
