import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { OpenWindow } from "./windowSelection";

type WindowId = Pick<OpenWindow, "hwnd" | "pid">;
export interface Snapshot {
  revision: number;
  phase: "Idle" | "ActiveFocus" | "Aborted";
  goal: string;
  blocked_count: number;
  blocked: { process_name: string; window_title: string } | null;
  needs_replacement: boolean;
  challenge: string | null;
  error: string | null;
}

export type Action =
  | { type: "start"; goal: string; windows: WindowId[] }
  | { type: "replace_window"; window: WindowId }
  | { type: "type_challenge"; value: string }
  | { type: "snapshot" | "return_to_focus" | "open_emergency" | "close_emergency" | "begin_hold" | "cancel_hold" | "confirm_hold" };

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
