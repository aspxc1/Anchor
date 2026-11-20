import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { errorText } from "./session";

export function AutostartSettings() {
  const [enabled, setEnabled] = useState<boolean>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    let active = true;
    void invoke<boolean>("read_autostart").then(
      (enabled) => { if (active) setEnabled(enabled); },
      (reason) => { if (active) setError(errorText(reason)); },
    );
    return () => { active = false; };
  }, []);

  async function update(value: boolean) {
    setBusy(true);
    setError(undefined);
    try {
      setEnabled(await invoke<boolean>("set_autostart", { enabled: value }));
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel desktop-settings" aria-label="Windows startup">
      <label className="audio-toggle">
        <input type="checkbox" checked={enabled ?? false} disabled={enabled === undefined || busy}
          onChange={(event) => void update(event.target.checked)} />
        Start Anchor with Windows
      </label>
      {error && <p role="alert" className="action-error">{error}</p>}
    </section>
  );
}
