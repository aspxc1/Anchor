import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { DomainFields } from "./DomainFields";
import { errorText } from "./session";
import "./profiles.css";

export interface ProfileApp {
  process_name: string;
  executable_path: string;
  launch_if_closed: boolean;
  title_hint: string | null;
}
export interface ProfileDraft {
  name: string;
  color: string;
  default_goal: string;
  focus_minutes: number;
  break_minutes: number;
  hall_passes: number;
  ambient_sound: "none" | "brown" | "pink";
  ambient_volume: number;
  allowed_apps: ProfileApp[];
  allowed_domains: string[];
  allow_localhost_domains: boolean;
}
export type Profile = ProfileDraft & { id: string; created_at: string; last_used_at: string | null };

const emptyDraft: ProfileDraft = { name: "", color: "#6b8f80", default_goal: "", focus_minutes: 25, break_minutes: 5, hall_passes: 0, ambient_sound: "none", ambient_volume: 0.5, allowed_apps: [], allowed_domains: [], allow_localhost_domains: false };
const copyDraft = (draft: ProfileDraft): ProfileDraft => ({ ...draft, allowed_apps: draft.allowed_apps.map((app) => ({ ...app })), allowed_domains: [...draft.allowed_domains] });
const safeColor = (color: string) => /^#[0-9a-f]{6}$/i.test(color) ? color : "#6b8f80";

export function ProfileStrip({ setup, disabled, onLaunch }: { setup: ProfileDraft; disabled: boolean; onLaunch: () => void }) {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [editor, setEditor] = useState<{ id: string | null; draft: ProfileDraft }>();
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    let active = true;
    void invoke<Profile[]>("list_profiles").then((items) => { if (active) setProfiles(items); }, (reason) => { if (active) setError(errorText(reason)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, []);
  useEffect(() => { if (editor && dialog.current && !dialog.current.open) dialog.current.showModal(); }, [editor]);
  const change = (patch: Partial<ProfileDraft>) => setEditor((current) => current && ({ ...current, draft: { ...current.draft, ...patch } }));

  async function save() {
    if (!editor) return;
    setBusy(true); setError(undefined);
    try {
      const saved = await invoke<Profile>("save_profile", { id: editor.id, draft: editor.draft });
      setProfiles((items) => [...items.filter((item) => item.id !== saved.id), saved]);
      setEditor(undefined); setNotice("Profile saved.");
    } catch (reason) { setError(errorText(reason)); } finally { setBusy(false); }
  }
  async function remove(profile: Profile) {
    if (!window.confirm(`Delete profile “${profile.name}”?`)) return;
    setBusy(true); setError(undefined); setNotice(undefined);
    try { await invoke("delete_profile", { id: profile.id }); setProfiles((items) => items.filter((item) => item.id !== profile.id)); setNotice("Profile deleted."); }
    catch (reason) { setError(errorText(reason)); } finally { setBusy(false); }
  }
  async function launch(profile: Profile) {
    setBusy(true); setError(undefined); setNotice(undefined);
    try {
      onLaunch();
      const result = await invoke<{ warnings: string[] }>("launch_profile", { id: profile.id });
      setNotice(result.warnings.length ? result.warnings.join(" ") : `Started ${profile.name}.`);
    } catch (reason) { setError(errorText(reason)); } finally { setBusy(false); }
  }
  async function pickApp() {
    setBusy(true); setError(undefined);
    try {
      const app = await invoke<ProfileApp | null>("pick_profile_app");
      if (app) setEditor((current) => current && ({ ...current, draft: { ...current.draft, allowed_apps: [...current.draft.allowed_apps, app] } }));
    } catch (reason) { setError(errorText(reason)); } finally { setBusy(false); }
  }
  return <section className="profile-section" aria-labelledby="profiles-heading">
    <div className="profile-heading"><div><h2 id="profiles-heading">Focus profiles</h2><p>Save a setup and start it again in one click.</p></div><div className="profile-actions"><button className="quiet-button" disabled={disabled || busy} onClick={() => { setError(undefined); setEditor({ id: null, draft: { ...copyDraft(setup), name: "" } }); }}>Save Current Setup</button><button className="quiet-button" disabled={disabled || busy} onClick={() => { setError(undefined); setEditor({ id: null, draft: copyDraft(emptyDraft) }); }}>New Profile</button></div></div>
    {loading ? <p>Loading profiles…</p> : profiles.length === 0 ? <p className="profile-empty">No profiles yet. Save your current setup or create one to get started.</p> : <div className="profile-list">{profiles.map((profile) => <article className="profile-card" key={profile.id}>
      <div className="profile-card-title"><span className="profile-swatch" style={{ backgroundColor: safeColor(profile.color) }} aria-hidden="true" /><h3>{profile.name}</h3></div>
      <p>{profile.default_goal || "No default goal"}</p><p className="profile-meta">{profile.focus_minutes} min focus · {profile.break_minutes} min break · {profile.allowed_apps.length} {profile.allowed_apps.length === 1 ? "app" : "apps"}</p>
      <div className="profile-actions"><button className="quiet-button" disabled={disabled || busy} onClick={() => void launch(profile)}>Start</button><button className="text-button" disabled={disabled || busy} onClick={() => { setError(undefined); setEditor({ id: profile.id, draft: copyDraft(profile) }); }}>Edit</button><button className="text-button danger-button" disabled={disabled || busy} onClick={() => void remove(profile)}>Delete</button></div>
    </article>)}</div>}
    {notice && <p className="session-notice" role="status">{notice}</p>}{error && !editor && <p className="action-error" role="alert">{error}</p>}
    {editor && <dialog ref={dialog} className="profile-dialog" onCancel={(event) => { if (busy) event.preventDefault(); else setEditor(undefined); }} onClose={() => setEditor(undefined)}><form onSubmit={(event) => { event.preventDefault(); void save(); }}>
      <div className="profile-dialog-heading"><h2>{editor.id ? "Edit Profile" : "New Profile"}</h2><button type="button" className="text-button" disabled={busy} onClick={() => setEditor(undefined)} aria-label="Close profile editor">Close</button></div>
      <div className="profile-form-grid"><label className="field"><span>Name</span><input autoFocus required maxLength={100} value={editor.draft.name} onChange={(e) => change({ name: e.target.value })} /></label><label className="field"><span>Color</span><input type="color" value={safeColor(editor.draft.color)} onChange={(e) => change({ color: e.target.value })} /></label></div>
      <label className="field"><span>Default goal</span><input value={editor.draft.default_goal} onChange={(e) => change({ default_goal: e.target.value })} /></label>
      <div className="profile-form-grid profile-numbers"><label className="field"><span>Focus minutes</span><input type="number" min="1" required value={editor.draft.focus_minutes} onChange={(e) => change({ focus_minutes: Number(e.target.value) })} /></label><label className="field"><span>Break minutes</span><input type="number" min="0" required value={editor.draft.break_minutes} onChange={(e) => change({ break_minutes: Number(e.target.value) })} /></label><label className="field"><span>Hall passes</span><input type="number" min="0" required value={editor.draft.hall_passes} onChange={(e) => change({ hall_passes: Number(e.target.value) })} /></label></div>
      <div className="profile-form-grid"><label className="field"><span>Ambient sound</span><select value={editor.draft.ambient_sound} onChange={(e) => change({ ambient_sound: e.target.value as ProfileDraft["ambient_sound"] })}><option value="none">None</option><option value="brown">Brown noise</option><option value="pink">Pink noise</option></select></label><label className="field"><span>Volume</span><input type="range" min="0" max="1" step="0.01" value={editor.draft.ambient_volume} onChange={(e) => change({ ambient_volume: Number(e.target.value) })} /></label></div>
      <fieldset className="profile-apps"><legend>Allowed apps</legend>{editor.draft.allowed_apps.length === 0 && <p>No apps added.</p>}{editor.draft.allowed_apps.map((app, index) => <div className="profile-app" key={`${app.executable_path}-${index}`}><div><strong>{app.process_name}</strong><small>{app.executable_path}</small><label><input type="checkbox" checked={app.launch_if_closed} onChange={(e) => change({ allowed_apps: editor.draft.allowed_apps.map((item, i) => i === index ? { ...item, launch_if_closed: e.target.checked } : item) })} /> Launch if closed</label></div><button type="button" className="text-button" onClick={() => change({ allowed_apps: editor.draft.allowed_apps.filter((_, i) => i !== index) })} aria-label={`Remove ${app.process_name}`}>Remove</button></div>)}<button type="button" className="quiet-button" disabled={busy} onClick={() => void pickApp()}>Browse for app</button></fieldset>
      <DomainFields domains={editor.draft.allowed_domains} allowLocalhost={editor.draft.allow_localhost_domains} changeDomains={(allowed_domains) => change({ allowed_domains })} changeLocalhost={(allow_localhost_domains) => change({ allow_localhost_domains })} />
      {error && <p className="action-error" role="alert">{error}</p>}<div className="profile-dialog-actions"><button type="button" className="text-button" disabled={busy} onClick={() => setEditor(undefined)}>Cancel</button><button className="primary-button" disabled={busy}>{busy ? "Saving…" : "Save Profile"}</button></div>
    </form></dialog>}
  </section>;
}
