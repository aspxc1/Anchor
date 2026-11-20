import { invoke } from "@tauri-apps/api/core";
import { Anchor, ArrowUpRight } from "lucide-react";
import { useEffect, useState } from "react";
import { EmergencyUnlock } from "./EmergencyUnlock";
import { countdown, errorText, useSession } from "./session";
import type { OpenWindow } from "./windowSelection";

export function Curtain() {
  const { snapshot, error, act } = useSession();
  const [replacements, setReplacements] = useState<OpenWindow[]>();
  const [discoveryError, setDiscoveryError] = useState<string>();

  useEffect(() => {
    if (!snapshot?.needs_replacement) setReplacements(undefined);
  }, [snapshot?.needs_replacement]);

  async function findReplacement() {
    setDiscoveryError(undefined);
    try {
      setReplacements(await invoke<OpenWindow[]>("list_open_windows"));
    } catch (reason) {
      setDiscoveryError(errorText(reason));
    }
  }

  if (!snapshot) return error && <p className="action-error" role="alert">{error}</p>;
  return (
    <main className="curtain-layout">
      {snapshot.challenge ? <EmergencyUnlock key={snapshot.challenge} challenge={snapshot.challenge} /> : (
        <section className="curtain-content">
          <span className="brand-mark"><Anchor size={21} aria-hidden="true" /></span>
          <h1>{snapshot.needs_replacement ? "Your allowed windows have closed" : `${snapshot.blocked?.process_name ?? "This window"} is outside your focus allowlist`}</h1>
          {snapshot.blocked?.window_title && <p className="blocked-title">{snapshot.blocked.window_title}</p>}
          <p className="curtain-goal">{snapshot.goal} <span className="time-chip">{countdown(snapshot.seconds_left)} remaining</span></p>
          {snapshot.needs_replacement ? (
            <>
              <button className="primary-button" onClick={() => void findReplacement()}>Find a replacement window</button>
              {replacements && (replacements.length ? (
                <ul className="replacement-list" aria-label="Replacement windows">
                  {replacements.map(({ title, hwnd, pid }) => (
                    <li key={hwnd}><button className="quiet-button" onClick={() => void act({ type: "replace_window", window: { hwnd, pid } })}>{title}</button></li>
                  ))}
                </ul>
              ) : <p>Open a window, then search again.</p>)}
            </>
          ) : <button className="primary-button" onClick={() => void act({ type: "return_to_focus" })}>Return to Focus <ArrowUpRight size={17} aria-hidden="true" /></button>}
          {snapshot.phase === "ActiveFocus" && snapshot.hall_passes_remaining > 0 && (
            <button className="quiet-button" onClick={() => void act({ type: "use_hall_pass" })}>
              Use 60s Hall Pass ({snapshot.hall_passes_remaining} remaining)
            </button>
          )}
          {(error || discoveryError || snapshot.error) && <p className="action-error" role="alert">{error || discoveryError || snapshot.error}</p>}
          <button className="text-button emergency-link" onClick={() => void act({ type: "open_emergency" })}>Emergency Unlock</button>
        </section>
      )}
    </main>
  );
}
