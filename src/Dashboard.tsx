import { invoke } from "@tauri-apps/api/core";
import { Anchor, Check, History, Layers2, Monitor, NotebookPen, PanelsTopLeft, RefreshCw, Search, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { AutostartSettings } from "./AutostartSettings";
import { BridgeSettings } from "./BridgeSettings";
import { DomainFields } from "./DomainFields";
import { ProfileStrip, type ProfileDraft } from "./Profiles";
import { NotesLibrary, SessionHistory } from "./Library";
import { prepareAudio, useAmbientAudio } from "./ambient";
import { countdown, phaseName, useSession, type Audio } from "./session";
import { filterWindows, retainSelection, type OpenWindow } from "./windowSelection";

export function Dashboard() {
  const { snapshot, error: sessionError, act } = useSession();
  const [page, setPage] = useState<"setup" | "history" | "notes">("setup");
  const [sessionId, setSessionId] = useState<string>();
  const [libraryRevision, setLibraryRevision] = useState(0);
  const finishedId = snapshot?.last_finished_session;
  const notesRevision = libraryRevision + (snapshot?.notes_revision ?? 0);
  useEffect(() => {
    if (finishedId) {
      setSessionId(finishedId);
      setPage("history");
    }
  }, [finishedId]);

  const audioError = useAmbientAudio(snapshot);
  const [audioWarmupError, setAudioWarmupError] = useState<string>();
  const [audio, setAudio] = useState<Audio>({ enabled: false, volume: 22, noise: "brown" });
  const [hotkey, setHotkey] = useState("Ctrl+Shift+Space");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const loadedPreferences = useRef(false);
  const [preset, setPreset] = useState("25/5");
  const [focusMinutes, setFocusMinutes] = useState(25);
  const [breakMinutes, setBreakMinutes] = useState(5);
  const [hallPasses, setHallPasses] = useState(3);
  const [domains, setDomains] = useState<string[]>([]);
  const [allowLocalhost, setAllowLocalhost] = useState(true);
  const [breakDomains, setBreakDomains] = useState<string[]>([]);
  const [breakLocalhost, setBreakLocalhost] = useState(true);
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

  useEffect(() => {
    if (!snapshot || loadedPreferences.current) return;
    loadedPreferences.current = true;
    const defaults = snapshot.preferences;
    setFocusMinutes(defaults.focus_minutes);
    setBreakMinutes(defaults.break_minutes);
    setHallPasses(defaults.hall_passes);
    setPreset(defaults.focus_minutes === 25 && defaults.break_minutes === 5 ? "25/5" :
      defaults.focus_minutes === 50 && defaults.break_minutes === 10 ? "50/10" : "custom");
    setHotkey(defaults.hotkey);
  }, [snapshot]);

  const savedAudio = snapshot?.preferences.audio;
  const savedEnabled = savedAudio?.enabled;
  const savedNoise = savedAudio?.noise;
  const savedVolume = savedAudio?.volume;
  useEffect(() => {
    if (savedEnabled !== undefined && savedNoise !== undefined && savedVolume !== undefined) {
      setAudio({ enabled: savedEnabled, noise: savedNoise, volume: savedVolume });
    }
  }, [savedEnabled, savedNoise, savedVolume]);

  function warmAudio() {
    setAudioWarmupError(undefined);
    void prepareAudio().catch((reason) => {
      console.error(reason);
      setAudioWarmupError("The WebView could not activate ambient audio.");
    });
  }

  const toggle = ({ hwnd, pid }: OpenWindow) => {
    setSelection((selected) => {
      const next = new Map(selected);
      if (next.has(hwnd)) next.delete(hwnd);
      else next.set(hwnd, pid);
      return next;
    });
  };

  const running = snapshot?.phase === "ActiveFocus" || snapshot?.phase === "HallPass" || snapshot?.phase === "Break";
  const onBreak = snapshot?.phase === "Break";
  const filtered = filterWindows(windows, search);
  const allowed = running && snapshot ? snapshot.allowed : windows.filter(({ hwnd }) => selection.has(hwnd));

  const activeDomains = snapshot?.allowed_domains;
  const activeLocalhost = snapshot?.allow_localhost_domains;
  const activeDomainKey = JSON.stringify(activeDomains ?? []);
  useEffect(() => {
    setBreakDomains(JSON.parse(activeDomainKey) as string[]);
    setBreakLocalhost(activeLocalhost ?? true);
  }, [activeDomainKey, activeLocalhost, onBreak]);

  const profileSetup: ProfileDraft = {
    name: "",
    color: "#c8f584",
    default_goal: goal,
    focus_minutes: focusMinutes,
    break_minutes: breakMinutes,
    hall_passes: hallPasses,
    ambient_sound: audio.enabled ? audio.noise : "none",
    ambient_volume: audio.volume / 100,
    allowed_apps: allowed.map(({ hwnd, executable_path, process_name, title }) => ({
      process_name,
      executable_path: executable_path ?? "",
      launch_if_closed: false,
      title_hint: windows.some((other) => other.hwnd !== hwnd && other.process_name === process_name
        && !allowed.some((selected) => selected.hwnd === other.hwnd)) ? title : null,
    })),
    allowed_domains: domains,
    allow_localhost_domains: allowLocalhost,
  };

  return (
    <div className="app-layout">
      <aside className="rail">
        <div className="brand">
          <span className="brand-mark"><Anchor size={19} aria-hidden="true" /></span>
          Anchor
        </div>
        <nav className="rail-nav" aria-label="Anchor">
          <button className="rail-item" aria-current={page === "setup" ? "page" : undefined} onClick={() => setPage("setup")}><Layers2 size={16} /> Session setup</button>
          <button className="rail-item" aria-current={page === "history" ? "page" : undefined} onClick={() => setPage("history")}><History size={16} /> History</button>
          <button className="rail-item" aria-current={page === "notes" ? "page" : undefined} onClick={() => setPage("notes")}><NotebookPen size={16} /> Brain Dumps</button>
        </nav>
      </aside>
      <main className="workspace">
        {(sessionError || snapshot?.error) && <p className="action-error" role="alert">{sessionError || snapshot?.error}</p>}
        {page === "history" ? <SessionHistory selected={sessionId} select={setSessionId} notesRevision={notesRevision} /> :
          page === "notes" ? <NotesLibrary revision={notesRevision} onChange={() => setLibraryRevision((value) => value + 1)} capture={() => void act({ type: "open_capture" })} /> : <>
        <header className="page-header settings-heading">
          <div>
            <h1>Session setup</h1>
            <p>Choose the windows you need for your next focus session.</p>
          </div>
          <button className="quiet-button" onClick={() => setSettingsOpen((open) => !open)} aria-expanded={settingsOpen}>Settings</button>
        </header>
        {(snapshot?.phase === "Aborted" || snapshot?.phase === "Completed") && (
          <p className="session-notice" role="status">{snapshot.phase === "Aborted" ? "Session unlocked" : "Session complete"}. {countdown(snapshot.actual_focus_seconds)} focused · {snapshot.blocked_count} blocked switches.</p>
        )}
        {!running && <ProfileStrip setup={profileSetup} disabled={running} onLaunch={warmAudio} />}
        {snapshot?.profile_warning && <p className="session-notice" role="status">{snapshot.profile_warning}</p>}
        {settingsOpen && <><BridgeSettings /><AutostartSettings /></>}
        {settingsOpen && (
          <form className="session-settings panel" onSubmit={(event) => { event.preventDefault(); void act({ type: "change_hotkey", hotkey }); }}>
            <label className="field"><span>Brain Dump shortcut</span><input value={hotkey} maxLength={80} required onChange={(event) => setHotkey(event.target.value)} /></label>
            <button type="submit" className="quiet-button">Save shortcut</button>
            <button type="button" className="quiet-button" onClick={() => void act({ type: "open_capture" })}><NotebookPen size={15} /> Open Brain Dump</button>
          </form>
        )}
        {snapshot?.shortcut_error && <p className="action-error" role="alert">{snapshot.shortcut_error}</p>}
        {(audioError || audioWarmupError) && <p className="action-error" role="alert">{audioError || audioWarmupError}</p>}
        {running && snapshot ? (
          <section className="session-settings panel" aria-label="Current session">
            <div className="current-session">
              <strong>{phaseName(snapshot.phase)} <span className="time-chip">{countdown(snapshot.seconds_left)}</span></strong>
              <p>{snapshot.goal}</p>
            </div>
            {onBreak && <>
              <button className="primary-button" onClick={() => void act({ type: "next_sprint" })}>Start Next Focus Sprint</button>
              <button className="quiet-button" onClick={() => void act({ type: "end_session" })}>End Session</button>
              <DomainFields domains={breakDomains} allowLocalhost={breakLocalhost} changeDomains={setBreakDomains} changeLocalhost={setBreakLocalhost} />
              <button className="quiet-button" onClick={() => void act({
                type: "update_domains", domains: breakDomains, allow_localhost: breakLocalhost,
              })}>Apply website list</button>
            </>}
            {settingsOpen && <div className="audio-settings">
              <AudioFields audio={audio} change={setAudio} />
              <button className="quiet-button" onClick={() => { warmAudio(); void act({ type: "change_audio", audio }); }}>Apply audio</button>
            </div>}
          </section>
        ) : <form className="session-settings panel" onSubmit={(event) => {
          event.preventDefault();
          warmAudio();
          void act({
            type: "start", goal, windows: allowed.map(({ hwnd, pid }) => ({ hwnd, pid })),
            focus_minutes: focusMinutes, break_minutes: breakMinutes, hall_passes: hallPasses, audio,
            allowed_domains: domains, allow_localhost_domains: allowLocalhost,
          });
        }}>
          <label className="field">
            <span>Session goal</span>
            <input value={goal} onChange={(event) => setGoal(event.target.value)} maxLength={240} placeholder="What will you work on?" required />
          </label>
          <label className="field">
            <span>Pomodoro</span>
            <select value={preset} onChange={(event) => {
              const value = event.target.value;
              setPreset(value);
              if (value === "25/5") { setFocusMinutes(25); setBreakMinutes(5); }
              if (value === "50/10") { setFocusMinutes(50); setBreakMinutes(10); }
            }}>
              <option value="25/5">25 / 5 minutes</option>
              <option value="50/10">50 / 10 minutes</option>
              <option value="custom">Custom</option>
            </select>
          </label>
          {preset === "custom" && <>
            <label className="field compact-field"><span>Focus (min)</span><input type="number" min={1} max={180} required value={focusMinutes || ""} onChange={(event) => setFocusMinutes(Number(event.target.value))} /></label>
            <label className="field compact-field"><span>Break (min)</span><input type="number" min={1} max={60} required value={breakMinutes || ""} onChange={(event) => setBreakMinutes(Number(event.target.value))} /></label>
          </>}
          <label className="field compact-field"><span>Hall Passes</span><input type="number" min={0} max={20} required value={hallPasses} onChange={(event) => setHallPasses(Number(event.target.value))} /></label>
          <DomainFields domains={domains} allowLocalhost={allowLocalhost} changeDomains={setDomains} changeLocalhost={setAllowLocalhost} />
          <div className="audio-settings">
            <AudioFields audio={audio} change={setAudio} />
            <button className="primary-button" disabled={!goal.trim() || allowed.length === 0}>Start focus</button>
          </div>
        </form>}
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
                    <div className="window-entry"><label className="window-row">
                      {!running && <input type="checkbox" checked={selection.has(window.hwnd)} onChange={() => toggle(window)} />}
                      <span className="window-icon"><PanelsTopLeft size={18} aria-hidden="true" /></span>
                      <span className="window-copy">
                        <span className="window-title" title={window.title}>{window.title}</span>
                        <span className="window-process">{window.process_name}</span>
                      </span>
                      {!running && <span className="check-indicator"><Check size={13} strokeWidth={3} aria-hidden="true" /></span>}
                    </label>
                    {onBreak && !allowed.some(({ hwnd, pid }) => hwnd === window.hwnd && pid === window.pid) && (
                      <button className="quiet-button row-action" aria-label={`Add ${window.title} to allowlist`} onClick={() => void act({ type: "add_window", window })}>Add</button>
                    )}
                    </div>
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
              {!running && allowed.length > 0 && <button className="text-button" onClick={() => setSelection(new Map())}>Clear</button>}
            </div>
            {allowed.length === 0 ? (
              <p className="selection-empty">Select an open window to add it to your allowlist.</p>
            ) : (
              <ul className="selection-list">
                {allowed.map((window) => (
                  <li key={window.hwnd}>
                    <span title={window.title}>{window.title}</span>
                    {!running && <button className="icon-button" aria-label={`Remove ${window.title}`} onClick={() => toggle(window)}><X size={15} aria-hidden="true" /></button>}
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
        </>}
      </main>
    </div>
  );
}

function AudioFields({ audio, change }: { audio: Audio; change: (next: Audio) => void }) {
  return (
    <>
      <label className="audio-toggle"><input type="checkbox" checked={audio.enabled} onChange={(event) => change({ ...audio, enabled: event.target.checked })} /> Ambient audio</label>
      {audio.enabled && <>
        <label className="field compact-field"><span>Noise</span>
          <select value={audio.noise} onChange={(event) => change({ ...audio, noise: event.target.value === "pink" ? "pink" : "brown" })}>
            <option value="brown">Brown noise</option>
            <option value="pink">Pink noise</option>
          </select>
        </label>
        <label className="volume-field"><span>Volume {audio.volume}%</span>
          <input type="range" min={0} max={100} value={audio.volume} onChange={(event) => change({ ...audio, volume: Number(event.target.value) })} />
        </label>
      </>}
    </>
  );
}
