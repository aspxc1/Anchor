import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { OpenWindow } from "./windowSelection";

type WindowId = Pick<OpenWindow, "hwnd" | "pid">;
export interface Audio {
  enabled: boolean;
  noise: "brown" | "pink";
  volume: number;
}

export interface Preferences {
  hotkey: string;
  focus_minutes: number;
  break_minutes: number;
  hall_passes: number;
  audio: Audio;
}

export interface Snapshot {
  revision: number;
  last_finished_session: string | null;
  notes_revision: number;
  capture_open: boolean;
  preferences: Preferences;
  shortcut_error: string | null;
  phase: "Idle" | "ActiveFocus" | "HallPass" | "Break" | "Completed" | "Aborted";
  seconds_left: number;
  hall_passes_remaining: number;
  actual_focus_seconds: number;
  allowed: OpenWindow[];
  goal: string;
  blocked_count: number;
  blocked: { process_name: string; window_title: string } | null;
  needs_replacement: boolean;
  challenge: string | null;
  error: string | null;
}

export type Action =
  | { type: "start"; goal: string; windows: WindowId[]; focus_minutes: number; break_minutes: number; hall_passes: number; audio: Audio }
  | { type: "save_capture"; content: string }
  | { type: "change_hotkey"; hotkey: string }
  | { type: "change_audio"; audio: Audio }
  | { type: "replace_window" | "add_window"; window: WindowId }
  | { type: "type_challenge"; value: string }
  | { type: "snapshot" | "return_to_focus" | "open_emergency" | "close_emergency" | "begin_hold" | "cancel_hold" | "confirm_hold" | "use_hall_pass" | "end_hall_pass" | "next_sprint" | "end_session" | "show_recovery" | "show_dashboard" | "open_capture" | "dismiss_capture" | "toggle_audio" };

export function send(action: Action): Promise<Snapshot> {
  return invoke<Snapshot>("session_command", { action });
}

export function errorText(reason: unknown): string {
  if (typeof reason === "string") return reason;
  if (reason instanceof Error) return reason.message;
  return "Anchor could not complete that action. Please try again.";
}

export function useSession() {
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!isTauri()) return;
    let mounted = true;
    const accept = (next: Snapshot) => {
      if (mounted) {
        setSnapshot((current) => !current || next.revision >= current.revision ? next : current);
      }
    };
    const subscription = listen<Snapshot>("session-state", ({ payload }) => accept(payload));
    void subscription.then(() => send({ type: "snapshot" })).then(accept).catch((reason) => {
      if (mounted) setError(errorText(reason));
    });
    return () => {
      mounted = false;
      void subscription.then((unlisten) => unlisten()).catch((reason) => console.error(reason));
    };
  }, []);

  async function act(action: Action) {
    setError(undefined);
    try {
      const next = await send(action);
      setSnapshot((current) => !current || next.revision >= current.revision ? next : current);
    } catch (reason) {
      setError(errorText(reason));
    }
  }

  return { snapshot, error, act };
}

export function countdown(seconds: number): string {
  return [Math.floor(seconds / 60), seconds % 60].map((part) => String(part).padStart(2, "0")).join(":");
}

export function phaseName(phase: Snapshot["phase"]) {
  switch (phase) {
    case "ActiveFocus": return "Focus";
    case "HallPass": return "Hall Pass";
    default: return phase;
  }
}
