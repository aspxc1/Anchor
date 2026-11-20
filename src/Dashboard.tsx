import { invoke } from "@tauri-apps/api/core";
import { Anchor, Check, Layers2, Monitor, PanelsTopLeft, RefreshCw, Search, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useSession } from "./session";
import { filterWindows, retainSelection, type OpenWindow } from "./windowSelection";

export function Dashboard() {
  const { snapshot, error: sessionError, act } = useSession();
  const [goal, setGoal] = useState("");
  const [windows, setWindows] = useState<OpenWindow[]>([]);
  const [selection, setSelection] = useState<ReadonlyMap<number, number>>(new Map());
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>();
  const requestId = useRef(0);

  const refresh = useCallback(async () => {
    const id = ++requestId.current;
    setLoading(true);
    setError(undefined);
    try {
      const discovered = await invoke<OpenWindow[]>("list_open_windows");
      if (id !== requestId.current) return;
      setWindows(discovered);
      setSelection((selected) => retainSelection(selected, discovered));
    } catch (reason) {
      if (id !== requestId.current) return;
      setError(typeof reason === "string" ? reason : "Window discovery is unavailable. Open Anchor as a desktop app and try again.");
    } finally {
      if (id === requestId.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    return () => { requestId.current++; };
  }, [refresh]);

  const toggle = ({ hwnd, pid }: OpenWindow) => {
    setSelection((selected) => {
      const next = new Map(selected);
      if (next.has(hwnd)) next.delete(hwnd);
      else next.set(hwnd, pid);
      return next;
    });
  };

  const filtered = filterWindows(windows, search);
  const allowed = windows.filter(({ hwnd }) => selection.has(hwnd));

  return (
    <div className="app-layout">
      <aside className="rail">
        <div className="brand">
          <span className="brand-mark"><Anchor size={19} aria-hidden="true" /></span>
          Anchor
        </div>
        <div className="rail-item" aria-current="page">
          <Layers2 size={16} aria-hidden="true" /> Session setup
        </div>
      </aside>
      <main className="workspace">
        <header className="page-header">
          <h1>Session setup</h1>
          <p>Choose the windows you need for your next focus session.</p>
        </header>
        {snapshot?.phase === "Aborted" && (
          <p className="session-notice" role="status">Session unlocked. Anchor recorded {snapshot.blocked_count} blocked switches.</p>
        )}
        {(sessionError || snapshot?.error) && <p className="action-error" role="alert">{sessionError || snapshot?.error}</p>}
        <form className="session-settings panel" onSubmit={(event) => {
          event.preventDefault();
          void act({ type: "start", goal, windows: allowed.map(({ hwnd, pid }) => ({ hwnd, pid })) });
        }}>
          <label className="field">
            <span>Session goal</span>
            <input value={goal} onChange={(event) => setGoal(event.target.value)} maxLength={240} placeholder="What will you work on?" required />
          </label>
          <button className="primary-button" disabled={!goal.trim() || allowed.length === 0 || snapshot?.phase === "ActiveFocus"}>Start focus</button>
        </form>
        <div className="grid min-h-0 items-start gap-5 lg:grid-cols-[minmax(0,1fr)_260px]">
          <section className="panel" aria-labelledby="open-windows" aria-busy={loading}>
            <div className="panel-header">
              <div>
                <h2 id="open-windows">Open windows</h2>
                <p aria-live="polite">{loading ? "Looking for windows…" : error ? "Discovery unavailable" : `${windows.length} available`}</p>
              </div>
              <button className="quiet-button" onClick={() => void refresh()} disabled={loading}>
                <RefreshCw size={15} className={loading ? "animate-spin" : ""} aria-hidden="true" /> Refresh
              </button>
            </div>
            <div className="list-tools">
              <label className="search">
                <Search size={17} aria-hidden="true" />
                <input aria-label="Search by window title or app" placeholder="Search windows" value={search} onChange={(event) => setSearch(event.target.value)} />
              </label>
            </div>
            {error && <p className="discovery-error" role="alert">{error}</p>}
            {filtered.length > 0 && (
              <ul className="window-list" aria-label="Available windows">
                {filtered.map((window) => (
                  <li key={window.hwnd}>
                    <label className="window-row">
                      <input type="checkbox" checked={selection.has(window.hwnd)} onChange={() => toggle(window)} />
                      <span className="window-icon"><PanelsTopLeft size={18} aria-hidden="true" /></span>
                      <span className="window-copy">
                        <span className="window-title" title={window.title}>{window.title}</span>
                        <span className="window-process">{window.process_name}</span>
                      </span>
                      <span className="check-indicator"><Check size={13} strokeWidth={3} aria-hidden="true" /></span>
                    </label>
                  </li>
                ))}
              </ul>
            )}
            {!loading && !error && filtered.length === 0 && (
              <div className="empty">
                <Monitor size={23} aria-hidden="true" />
                <strong>{search.trim() ? "No matches" : "No open windows"}</strong>
                <p>{search.trim() ? "Try another title or app name." : "Open a study app, then refresh the list."}</p>
              </div>
            )}
          </section>
          <section className="panel selection-panel" aria-labelledby="selected-windows">
            <div className="panel-header">
              <div>
                <h2 id="selected-windows">Allowlist</h2>
                <p aria-live="polite">{allowed.length} {allowed.length === 1 ? "window" : "windows"} selected</p>
              </div>
              {allowed.length > 0 && <button className="text-button" onClick={() => setSelection(new Map())}>Clear</button>}
            </div>
            {allowed.length === 0 ? (
              <p className="selection-empty">Select an open window to add it to your allowlist.</p>
            ) : (
              <ul className="selection-list">
                {allowed.map((window) => (
                  <li key={window.hwnd}>
                    <span title={window.title}>{window.title}</span>
                    <button className="icon-button" aria-label={`Remove ${window.title}`} onClick={() => toggle(window)}><X size={15} aria-hidden="true" /></button>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
      </main>
    </div>
  );
}
