import { invoke } from "@tauri-apps/api/core";
import { CalendarDays, ChevronRight, Clipboard, Clock3, FolderOpen, History, NotebookPen, RefreshCw, Search, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { errorText } from "./session";

interface Summary {
  id: string;
  goal: string;
  started_at: string;
  ended_at: string;
  planned_focus_minutes: number;
  actual_focus_seconds: number;
  status: "completed" | "aborted";
  hall_passes_used: number;
  blocked_count: number;
  note_count: number;
}

interface Note {
  id: number;
  session_id: string | null;
  created_at: string;
  content: string;
}

interface HistoryData {
  sessions: Summary[];
  streak: number;
  total_focus_seconds: number;
}

interface Detail {
  summary: Summary;
  blocked: { process_name: string; count: number }[];
  notes: Note[];
  allowed: { title: string; process_name: string }[];
}

const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

function dateTime(date: string) {
  return dateFormat.format(new Date(date));
}

function focusTime(seconds: number) {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor(seconds / 60) % 60;
  if (hours) return `${hours}h ${minutes}m`;
  if (minutes) return `${minutes}m ${seconds % 60}s`;
  return `${seconds}s`;
}

function useLibrary<T>(command: string, dataRevision: number, id?: string) {
  const [data, setData] = useState<T>();
  const [error, setError] = useState<string>();
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let current = true;
    setError(undefined);
    void invoke<T>(command, id ? { id } : {}).then(
      (response) => { if (current) setData(response); },
      (reason) => { if (current) setError(errorText(reason)); },
    );
    return () => { current = false; };
  }, [command, id, revision, dataRevision]);

  return { data, error, reload: () => setRevision((value) => value + 1) };
}

function LoadStatus({ error, retry }: { error: string | undefined; retry: () => void }) {
  return error ? (
    <div className="empty">
      <p role="alert" className="action-error">{error}</p>
      <button className="quiet-button" onClick={retry}><RefreshCw size={14} /> Retry</button>
    </div>
  ) : <p className="empty" role="status">Loading…</p>;
}

export function SessionHistory({ selected, select, notesRevision }: {
  selected: string | undefined;
  select: (id: string) => void;
  notesRevision: number;
}) {
  const { data, error, reload } = useLibrary<HistoryData>("session_history", notesRevision);
  const active = selected && data?.sessions.some((item) => item.id === selected) ? selected : data?.sessions[0]?.id;

  return (
    <>
      <header className="page-header">
        <h1>Session history</h1>
        <p>Review your focus time and the windows that interrupted it.</p>
      </header>
      {data && error && <p className="action-error" role="alert">{error} <button className="text-button" onClick={reload}>Retry</button></p>}
      {!data ? <section className="panel"><LoadStatus error={error} retry={reload} /></section> : <>
        <div className="metrics">
          <div className="panel metric"><Clock3 size={17} /><span>Total focus</span><strong>{focusTime(data.total_focus_seconds)}</strong></div>
          <div className="panel metric"><CalendarDays size={17} /><span>Daily streak</span><strong>{data.streak} {data.streak === 1 ? "day" : "days"}</strong></div>
          <div className="panel metric"><History size={17} /><span>Sessions</span><strong>{data.sessions.length}</strong></div>
        </div>
        {data.sessions.length === 0 ? (
          <section className="panel empty">
            <History size={24} />
            <strong>No sessions yet</strong>
            <p>Completed and unlocked sessions will appear here.</p>
          </section>
        ) : (
          <div className="history-layout">
            <section className="panel" aria-label="Past focus sessions">
              <div className="panel-header"><h2>Past sessions</h2></div>
              <ul className="session-list">
                {data.sessions.map((session) => (
                  <li key={session.id}>
                    <button onClick={() => select(session.id)} aria-pressed={session.id === active}>
                      <span>
                        <strong>{session.goal}</strong>
                        <small>{dateTime(session.ended_at)}</small>
                      </span>
                      <ChevronRight size={16} />
                    </button>
                  </li>
                ))}
              </ul>
            </section>
            {active && <SessionDetail key={active} id={active} revision={notesRevision} />}
          </div>
        )}
      </>}
    </>
  );
}

function SessionDetail({ id, revision }: { id: string; revision: number }) {
  const { data, error, reload } = useLibrary<Detail>("session_detail", revision, id);
  if (!data) return <section className="panel"><LoadStatus error={error} retry={reload} /></section>;
  const { summary, blocked, notes, allowed } = data;
  return (
    <section className="panel session-detail" aria-label="Session summary">
      <header>
        <span className={`session-status ${summary.status}`}>{summary.status === "completed" ? "Completed" : "Aborted"}</span>
        <h2>{summary.goal}</h2>
        <p>{dateTime(summary.started_at)} · {summary.planned_focus_minutes} min sprints</p>
      </header>
      <dl className="detail-metrics">
        <div><dt>Focused</dt><dd>{focusTime(summary.actual_focus_seconds)}</dd></div>
        <div><dt>Switches blocked</dt><dd>{summary.blocked_count}</dd></div>
        <div><dt>Hall Passes used</dt><dd>{summary.hall_passes_used}</dd></div>
      </dl>
      {error && <p className="action-error" role="alert">{error} <button className="text-button" onClick={reload}>Retry</button></p>}
      <h3>Blocked switches by app</h3>
      {blocked.length > 0 ? (
        <ul className="bar-list">
          {blocked.map(({ process_name, count }) => (
            <li key={process_name}>
              <span>{process_name}</span>
              <meter aria-label={`Switches blocked for ${process_name}`} min={0} max={blocked[0]?.count ?? 1} value={count} />
              <strong>{count}</strong>
            </li>
          ))}
        </ul>
      ) : <p>No switches blocked.</p>}
      <h3>Brain Dumps ({summary.note_count})</h3>
      {notes.length ? <ul className="summary-notes">{notes.map((note) => <li key={note.id}><time dateTime={note.created_at}>{dateTime(note.created_at)}</time><p>{note.content}</p></li>)}</ul>
        : <p>No notes captured.</p>}
      {allowed.length > 0 && <>
        <h3>Allowed windows</h3>
        <ul className="summary-apps">{allowed.map((app, index) => <li key={index} title={app.process_name}>{app.title}</li>)}</ul>
      </>}
    </section>
  );
}

export function NotesLibrary({ revision, onChange, capture }: { revision: number; onChange: () => void; capture: () => void }) {
  const { data, error, reload } = useLibrary<Note[]>("list_notes", revision);
  const [search, setSearch] = useState("");
  const [pendingDelete, setPendingDelete] = useState<number>();
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string>();
  const [actionError, setActionError] = useState<string>();
  const query = search.trim().toLocaleLowerCase();
  const notes = data?.filter((note) => note.content.toLocaleLowerCase().includes(query));

  async function remove(note: Note) {
    setBusy(true);
    setActionError(undefined);
    setStatus(undefined);
    try {
      const warning = await invoke<string | null>("delete_note", { id: note.id });
      if (warning) setActionError(warning);
      else setStatus("Note deleted.");
      setPendingDelete(undefined);
      onChange();
    } catch (reason) {
      setActionError(errorText(reason));
    } finally {
      setBusy(false);
    }
  }

  async function copy(note: Note) {
    setStatus(undefined);
    setActionError(undefined);
    try {
      await navigator.clipboard.writeText(note.content);
      setStatus("Note copied.");
    } catch (reason) {
      setActionError(`Could not copy the note. ${errorText(reason)}`);
    }
  }

  async function openFolder() {
    setActionError(undefined);
    try {
      await invoke("open_markdown_folder");
    } catch (reason) {
      setActionError(errorText(reason));
    }
  }

  return (
    <>
      <header className="page-header settings-heading">
        <div><h1>Brain Dumps</h1><p>Browse the thoughts you captured while studying.</p></div>
        <div className="library-actions">
          <button className="quiet-button" onClick={() => void openFolder()}><FolderOpen size={16} /> Open Markdown folder</button>
          <button className="primary-button" onClick={capture}><NotebookPen size={16} /> Capture</button>
        </div>
      </header>
      {status && <p className="session-notice" role="status">{status}</p>}
      {actionError && <p className="action-error" role="alert">{actionError}</p>}
      <section className="panel" aria-label="Captured notes">
        <div className="panel-header">
          <h2>{data ? `${data.length} ${data.length === 1 ? "note" : "notes"}` : "Notes"}</h2>
          <label className="search"><Search size={16} /><input placeholder="Search notes" aria-label="Search notes" value={search} onChange={(event) => setSearch(event.target.value)} /></label>
        </div>
        {notes && error && <p className="discovery-error" role="alert">{error} <button className="text-button" onClick={reload}>Retry</button></p>}
        {!notes ? <LoadStatus error={error} retry={reload} /> : notes.length === 0 ? (
          <div className="empty"><NotebookPen size={24} /><strong>{query ? "No matching notes" : "No notes yet"}</strong></div>
        ) : (
          <ul className="notes-list">
            {notes.map((note) => (
              <li key={note.id}>
                <div><time dateTime={note.created_at}>{dateTime(note.created_at)}</time><p>{note.content}</p></div>
                {pendingDelete === note.id ? (
                  <div className="library-actions">
                    <button className="quiet-button" onClick={() => setPendingDelete(undefined)} disabled={busy}>Cancel</button>
                    <button className="quiet-button danger-button" onClick={() => void remove(note)} disabled={busy}>Delete note</button>
                  </div>
                ) : (
                  <div className="library-actions">
                    <button className="icon-button" aria-label="Copy note" title="Copy note" onClick={() => void copy(note)}><Clipboard size={16} /></button>
                    <button className="icon-button" aria-label="Delete note" title="Delete note" onClick={() => setPendingDelete(note.id)} disabled={busy}><Trash2 size={16} /></button>
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  );
}
