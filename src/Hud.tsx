import { invoke } from "@tauri-apps/api/core";
import { GripVertical, LayoutDashboard, LockKeyhole, NotebookPen, Play, Square, Ticket, TriangleAlert, Volume2, VolumeX } from "lucide-react";
import { useEffect, useRef, useState, type PointerEvent } from "react";
import { countdown, errorText, phaseName, useSession } from "./session";

export function Hud() {
  const { snapshot, error, act } = useSession();
  const [dragError, setDragError] = useState<string>();
  const [warningVisible, setWarningVisible] = useState(false);
  const warning = snapshot?.profile_warning;
  const phase = snapshot?.phase;
  useEffect(() => {
    if (!warning || phase !== "ActiveFocus") return;
    setWarningVisible(true);
    const timer = setTimeout(() => setWarningVisible(false), 8000);
    return () => { clearTimeout(timer); setWarningVisible(false); };
  }, [warning, phase]);
  const pointer = useRef<number | null>(null);
  const frame = useRef<number | undefined>(undefined);
  const request = useRef(Promise.resolve());

  function drag(action: "begin" | "move" | "end" | "cancel") {
    request.current = request.current
      .then(() => invoke<void>("hud_drag", { action }))
      .catch((reason) => setDragError(errorText(reason)));
    return request.current;
  }

  function beginDrag(event: PointerEvent<HTMLElement>) {
    if (event.button !== 0 || pointer.current !== null) return;
    if ((event.target as HTMLElement).closest("button")) return;
    pointer.current = event.pointerId;
    event.currentTarget.setPointerCapture(event.pointerId);
    setDragError(undefined);
    void drag("begin");
  }

  function move(event: PointerEvent<HTMLElement>) {
    if (pointer.current !== event.pointerId || frame.current !== undefined) return;
    frame.current = requestAnimationFrame(() => {
      void drag("move").finally(() => { frame.current = undefined; });
    });
  }

  function endDrag(event: PointerEvent<HTMLElement>, cancelled = false) {
    if (pointer.current !== event.pointerId) return;
    pointer.current = null;
    if (frame.current !== undefined) {
      cancelAnimationFrame(frame.current);
      frame.current = undefined;
    }
    void drag(cancelled ? "cancel" : "end");
  }

  if (!snapshot) return error && <div className="hud" role="alert">{error}</div>;
  const pass = snapshot.phase === "HallPass";
  const onBreak = snapshot.phase === "Break";
  const focus = snapshot.phase === "ActiveFocus";
  return (
    <main className={`hud ${pass ? "hud-pass" : onBreak ? "hud-break" : ""}`}
      onPointerDown={beginDrag} onPointerMove={move}
      onPointerUp={endDrag} onLostPointerCapture={(event) => endDrag(event, true)} onPointerCancel={(event) => endDrag(event, true)}>
      <div className="hud-grip" title="Drag to move HUD">
        <GripVertical size={16} aria-hidden="true" />
      </div>
      <span className="hud-phase">{phaseName(snapshot.phase)}</span>
      <time className="hud-time">{countdown(snapshot.seconds_left)}</time>
      {warningVisible && warning
        ? <span className="hud-goal hud-error" role="status" title={warning}>Some profile apps are unavailable</span>
        : <span className="hud-goal" title={snapshot.goal}>{snapshot.goal}</span>}
      {(error || dragError || snapshot.error) && <span className="hud-error" role="alert" title={error || dragError || snapshot.error || undefined}> <TriangleAlert size={15} /> </span>}
      {focus && snapshot.needs_replacement && <button className="icon-button" aria-label="Pick a replacement window" title="Pick a replacement window" onClick={() => void act({ type: "show_recovery" })}><TriangleAlert size={16} /></button>}
      {focus && <button className="icon-button hud-ticket" aria-label={`Use Hall Pass, ${snapshot.hall_passes_remaining} remaining`} title="Use 60s Hall Pass" disabled={snapshot.hall_passes_remaining === 0} onClick={() => void act({ type: "use_hall_pass" })}><Ticket size={15} /><span>{snapshot.hall_passes_remaining}</span></button>}
      {pass && <button className="hud-small-button" onClick={() => void act({ type: "end_hall_pass" })}>End Pass Early</button>}
      {onBreak && <>
        <button className="icon-button" aria-label="Start Next Focus Sprint" title="Start Next Focus Sprint" onClick={() => void act({ type: "next_sprint" })}><Play size={15} /></button>
        <button className="icon-button" aria-label="End Session" title="End Session" onClick={() => void act({ type: "end_session" })}><Square size={14} /></button>
        <button className="icon-button" aria-label="Show dashboard to add a window" title="Show Dashboard" onClick={() => void act({ type: "show_dashboard" })}><LayoutDashboard size={15} /></button>
      </>}
      <button className="icon-button" aria-label="Brain Dump" title={`Brain Dump (${snapshot.preferences.hotkey})`} onClick={() => void act({ type: "open_capture" })}><NotebookPen size={15} /></button>
      <button className="icon-button" aria-label={snapshot.preferences.audio.enabled ? "Mute ambient audio" : "Unmute ambient audio"} title={snapshot.preferences.audio.enabled ? "Mute ambient audio" : "Unmute ambient audio"} onClick={() => void act({ type: "toggle_audio" })}>
        {snapshot.preferences.audio.enabled ? <Volume2 size={15} /> : <VolumeX size={15} />}
      </button>
      <button className="icon-button" aria-label="Emergency Unlock" title="Emergency Unlock" onClick={() => void act({ type: "open_emergency" })}><LockKeyhole size={15} /></button>
    </main>
  );
}
