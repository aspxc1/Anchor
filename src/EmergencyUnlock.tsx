import { ShieldAlert } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { errorText, send } from "./session";

export function EmergencyUnlock({ challenge }: { challenge: string }) {
  const [value, setValue] = useState("");
  const [holding, setHolding] = useState(false);
  const [error, setError] = useState<string>();
  const activeHold = useRef<symbol | null>(null);
  const timeout = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const cancel = useCallback(() => {
    if (!activeHold.current) return;
    activeHold.current = null;
    clearTimeout(timeout.current);
    setHolding(false);
    void send({ type: "cancel_hold" }).catch((reason) => setError(errorText(reason)));
  }, []);

  useEffect(() => {
    window.addEventListener("blur", cancel);
    return () => {
      window.removeEventListener("blur", cancel);
      cancel();
    };
  }, [cancel]);

  async function run(action: Parameters<typeof send>[0]) {
    setError(undefined);
    try {
      await send(action);
    } catch (reason) {
      setError(errorText(reason));
    }
  }

  async function begin() {
    if (activeHold.current) return;
    const attempt = Symbol();
    activeHold.current = attempt;
    setError(undefined);
    try {
      await send({ type: "begin_hold" });
      if (activeHold.current !== attempt) return;
      setHolding(true);
      timeout.current = setTimeout(() => {
        activeHold.current = null;
        setHolding(false);
        void run({ type: "confirm_hold" });
      }, 10_000);
    } catch (reason) {
      if (activeHold.current === attempt) {
        activeHold.current = null;
        setError(errorText(reason));
      }
    }
  }

  return (
    <section className="unlock" role="dialog" aria-modal="true" aria-labelledby="unlock-heading"
      onKeyDown={(event) => {
        if (event.key === "Escape") { cancel(); void run({ type: "close_emergency" }); }
      }}>
      <ShieldAlert size={28} className="unlock-icon" aria-hidden="true" />
      <h2 id="unlock-heading">Emergency Unlock</h2>
      <p>This ends the focus session. Type the code or hold the unlock button for 10 seconds.</p>
      <form onSubmit={(event) => { event.preventDefault(); void run({ type: "type_challenge", value }); }}>
        <code className="challenge">{challenge}</code>
        <label className="field">
          <span>Unlock code</span>
          <input autoFocus value={value} onChange={(event) => setValue(event.target.value)}
            maxLength={24} autoComplete="off" autoCapitalize="none" spellCheck={false}
            onPaste={(event) => event.preventDefault()} onDrop={(event) => event.preventDefault()} />
        </label>
        <button className="quiet-button" type="submit" disabled={value.length !== 24}>Submit code</button>
      </form>
      <button type="button" className={`hold-button ${holding ? "holding" : ""}`}
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          event.currentTarget.setPointerCapture(event.pointerId);
          void begin();
        }}
        onPointerMove={(event) => {
          const rect = event.currentTarget.getBoundingClientRect();
          if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) cancel();
        }}
        onPointerUp={cancel} onPointerCancel={cancel} onLostPointerCapture={cancel} onBlur={cancel}
        onKeyDown={(event) => {
          if (event.key === " " || event.key === "Enter") { event.preventDefault(); if (!event.repeat) void begin(); }
        }}
        onKeyUp={(event) => { if (event.key === " " || event.key === "Enter") cancel(); }}>
        <span>{holding ? "Keep holding…" : "Hold to Unlock Session"}</span>
      </button>
      {error && <p className="action-error" role="alert">{error}</p>}
      <button className="text-button" onClick={() => { cancel(); void run({ type: "close_emergency" }); }}>Back to session</button>
    </section>
  );
}
