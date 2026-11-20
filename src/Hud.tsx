import { getCurrentWindow, PhysicalPosition, primaryMonitor } from "@tauri-apps/api/window";
import { GripVertical, LayoutDashboard, LockKeyhole, NotebookPen, Play, Square, Ticket, TriangleAlert, Volume2, VolumeX } from "lucide-react";
import { useRef, useState, type PointerEvent } from "react";
import { countdown, errorText, phaseName, useSession } from "./session";

interface Drag {
  start: number;
  position: PhysicalPosition;
  min: number;
  max: number;
  scale: number;
}

export function Hud() {
  const { snapshot, error, act } = useSession();
  const [dragError, setDragError] = useState<string>();
  const drag = useRef<Drag | null>(null);
  const pointer = useRef<number | null>(null);
  const frame = useRef<number | undefined>(undefined);

  async function beginDrag(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    const { screenX, pointerId } = event;
    event.currentTarget.setPointerCapture(pointerId);
    pointer.current = pointerId;
    setDragError(undefined);
    const window = getCurrentWindow();
    try {
      const [position, size, monitor] = await Promise.all([window.outerPosition(), window.outerSize(), primaryMonitor()]);
      if (!monitor) throw new Error("Windows did not report the primary monitor.");
      if (pointer.current !== pointerId) return;
      const min = monitor.workArea.position.x;
      drag.current = { start: screenX, position, min, max: Math.max(min, min + monitor.workArea.size.width - size.width), scale: monitor.scaleFactor };
    } catch (reason) {
      setDragError(errorText(reason));
    }
  }

  function move(event: PointerEvent<HTMLDivElement>) {
    const active = drag.current;
    if (!active || pointer.current !== event.pointerId) return;
    const delta = Math.round((event.screenX - active.start) * active.scale);
    const x = Math.min(active.max, Math.max(active.min, active.position.x + delta));
    if (frame.current !== undefined) cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(() => {
      frame.current = undefined;
      void getCurrentWindow().setPosition(new PhysicalPosition(x, active.position.y))
        .catch((reason) => setDragError(errorText(reason)));
    });
  }

  function endDrag() {
    pointer.current = null;
    drag.current = null;
  }

  if (!snapshot) return error && <div className="hud" role="alert">{error}</div>;
  const pass = snapshot.phase === "HallPass";
  const onBreak = snapshot.phase === "Break";
  const focus = snapshot.phase === "ActiveFocus";
  return (
    <main className={`hud ${pass ? "hud-pass" : onBreak ? "hud-break" : ""}`}>
      <div className="hud-grip" title="Move HUD horizontally"
        onPointerDown={(event) => void beginDrag(event)} onPointerMove={move}
        onPointerUp={endDrag} onLostPointerCapture={endDrag} onPointerCancel={endDrag}>
        <GripVertical size={16} aria-hidden="true" />
      </div>
      <span className="hud-phase">{phaseName(snapshot.phase)}</span>
      <time className="hud-time">{countdown(snapshot.seconds_left)}</time>
      <span className="hud-goal" title={snapshot.goal}>{snapshot.goal}</span>
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
