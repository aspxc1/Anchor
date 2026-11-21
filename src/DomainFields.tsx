import { Globe2, Plus, X } from "lucide-react";
import { useState } from "react";

interface Props {
  domains: string[];
  allowLocalhost: boolean;
  changeDomains: (domains: string[]) => void;
  changeLocalhost: (allow: boolean) => void;
}

function normalizePattern(raw: string): string {
  let value = raw.trim();
  if (!value) return "";
  if (/^https?:\/\//i.test(value)) {
    value = value.replace(/^https?:\/\//i, "");
  }
  const hashIndex = value.indexOf("#");
  if (hashIndex >= 0) value = value.slice(0, hashIndex);
  const split = value.search(/[/?]/);
  let host = (split < 0 ? value : value.slice(0, split)).replace(/:\d+$/, "");
  const suffix = split < 0 ? "" : value.slice(split);
  const wildcard = host.startsWith("*.") ? "*." : "";
  if (wildcard) host = host.slice(2);
  host = host.toLowerCase().replace(/\.+$/, "").replace(/^www\./, "");
  if (!host) return raw.trim();
  const cleanSuffix = suffix === "/" || suffix === "/*" ? "" : suffix;
  return `${wildcard}${host}${cleanSuffix}`;
}

export function DomainFields({ domains, allowLocalhost, changeDomains, changeLocalhost }: Props) {
  const [input, setInput] = useState("");

  function add(patterns: string[]) {
    const tags = patterns.map(normalizePattern).filter(Boolean);
    changeDomains([...new Set([...domains, ...tags])].slice(0, 100));
    setInput("");
  }

  return (
    <fieldset className="domain-fields">
      <legend><Globe2 size={15} aria-hidden="true" /> Allowed websites</legend>
      <p>Leave this empty to allow every website.</p>
      {domains.length > 0 && <ul className="domain-tags" aria-label="Allowed website patterns">
        {domains.map((pattern) => (
          <li key={pattern}>
            <span>{pattern}</span>
            <button type="button" aria-label={`Remove ${pattern}`} onClick={() => changeDomains(domains.filter((item) => item !== pattern))}><X size={13} /></button>
          </li>
        ))}
      </ul>}
      <div className="domain-entry">
        <label className="field">
          <span className="sr-only">Website pattern</span>
          <input value={input} maxLength={500} placeholder="docs.rs or *.mit.edu"
            onChange={(event) => setInput(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === ",") {
                event.preventDefault();
                add(input.split(","));
              }
            }} />
        </label>
        <button type="button" className="quiet-button" disabled={!input.trim()} onClick={() => add(input.split(","))}><Plus size={14} /> Add</button>
        <button type="button" className="text-button domain-preset" onClick={() => add(["docs.rs", "developer.mozilla.org", "en.wikipedia.org"])}>Docs &amp; Reference</button>
      </div>
      <label className="audio-toggle"><input type="checkbox" checked={allowLocalhost} onChange={(event) => changeLocalhost(event.target.checked)} /> Allow localhost / 127.0.0.1</label>
    </fieldset>
  );
}
