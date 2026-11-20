import { NotebookPen } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { errorText, send, useSession } from "./session";

export function BrainDump() {
  const { snapshot } = useSession();
  const [content, setContent] = useState("");
  const [error, setError] = useState<string>();
  const [saving, setSaving] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const savingRef = useRef(false);
  const open = snapshot?.capture_open === true;

  useEffect(() => {
    if (!open) return;
    setContent("");
    setError(undefined);
    input.current?.focus();
    const focus = () => input.current?.focus();
    const blur = () => {
      if (!savingRef.current) {
        void send({ type: "dismiss_capture" }).catch((reason) => setError(errorText(reason)));
      }
    };
    window.addEventListener("focus", focus);
    window.addEventListener("blur", blur);
    return () => {
      window.removeEventListener("focus", focus);
      window.removeEventListener("blur", blur);
    };
  }, [open]);

  if (!open) return null;

  async function save() {
    if (savingRef.current || !content.trim()) return;
    savingRef.current = true;
    setSaving(true);
    setError(undefined);
    try {
      await send({ type: "save_capture", content });
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  }

  return (
    <main className="capture">
      <form onSubmit={(event) => { event.preventDefault(); void save(); }}>
        <NotebookPen size={20} aria-hidden="true" />
        <input ref={input} aria-label="Brain dump note" placeholder="Capture a thought…" maxLength={1000}
          value={content} readOnly={saving} onChange={(event) => setContent(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape" && !savingRef.current) {
              event.preventDefault();
              void send({ type: "dismiss_capture" }).catch((reason) => setError(errorText(reason)));
            }
          }} />
      </form>
      {error ? <p role="alert" className="capture-error" title={error}>{error}</p> : <p><kbd>Enter</kbd> Save <kbd>Esc</kbd> Dismiss</p>}
    </main>
  );
}
