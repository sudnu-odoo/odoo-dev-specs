import { useEffect, useState, type FormEvent, type KeyboardEvent } from "react";
import type { ProjectInfo } from "../lib/types";

export interface NewSessionDialogProps {
  clients: ProjectInfo[];
  /** The client of the open session: offered first. */
  current?: string;
  /** `isNew`: the client is created first. Rejects with a message for the person. */
  onStart: (client: string, isNew: boolean) => Promise<void>;
  onCancel: () => void;
}

/** One line of the list: an existing client, or the one typed, to create. */
type Choice = { client: ProjectInfo; isNew: false } | { name: string; isNew: true };

const id = (c: Choice) => `client-${c.isNew ? "new" : c.client.name}`;

/** When a client was last worked on: its most recent session. */
const lastUsed = (p: ProjectInfo) => Math.max(0, ...p.sessions.map((s) => s.updated_at));

/**
 * Ctrl+N: a new session, for which client? Type to find one (the open session's
 * client, then the most recently worked on, come first) or to add a new one; the
 * arrow keys and Enter do the rest, so it stays quick with many clients.
 */
export default function NewSessionDialog({ clients, current, onStart, onCancel }: NewSessionDialogProps) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  const typed = query.trim();
  const exact = (c: ProjectInfo) => !!typed && c.name.toLowerCase() === typed.toLowerCase();
  const words = typed.toLowerCase().split(/\s+/).filter(Boolean);
  const choices: Choice[] = [...clients]
    .sort(
      (a, b) =>
        Number(exact(b)) - Number(exact(a)) ||
        Number(b.name === current) - Number(a.name === current) ||
        lastUsed(b) - lastUsed(a),
    )
    .filter((c) => words.every((w) => c.name.toLowerCase().includes(w)))
    .map((client) => ({ client, isNew: false }));
  if (typed && !clients.some(exact)) {
    choices.push({ name: typed, isNew: true });
  }
  const selected = choices[Math.min(active, choices.length - 1)];

  // Keep the chosen line in view while moving with the arrow keys.
  const selectedId = selected ? id(selected) : null;
  useEffect(() => {
    if (selectedId) document.getElementById(selectedId)?.scrollIntoView?.({ block: "nearest" });
  }, [selectedId]);

  async function start(choice: Choice | undefined, e?: FormEvent) {
    e?.preventDefault();
    if (!choice || starting) return;
    setStarting(true);
    try {
      await onStart(choice.isNew ? choice.name : choice.client.name, choice.isNew);
    } catch (err) {
      setError(String(err));
      setStarting(false);
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") onCancel();
    else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const step = e.key === "ArrowDown" ? 1 : -1;
      setActive((i) => Math.max(0, Math.min(choices.length - 1, Math.min(i, choices.length - 1) + step)));
    }
  }

  return (
    <div
      className="settings-overlay"
      role="dialog"
      aria-label="New session"
      onMouseDown={(e) => e.target === e.currentTarget && onCancel()}
    >
      <form className="dialog new-session" onSubmit={(e) => start(selected, e)} onKeyDown={onKeyDown}>
        <h2>New session</h2>
        <input
          autoFocus
          className="client-search"
          role="combobox"
          aria-label="Client"
          aria-expanded="true"
          aria-controls="client-list"
          aria-activedescendant={selected ? id(selected) : undefined}
          placeholder={clients.length ? "Find a client, or type a new name" : "Client name, e.g. Deco Addict"}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
            setError(null);
          }}
        />
        <div className="client-list" id="client-list" role="listbox" aria-label="Clients">
          {choices.map((c, i) => (
            <div
              key={id(c)}
              id={id(c)}
              role="option"
              aria-selected={c === selected}
              className={`client-option${c.isNew ? " client-new" : ""}`}
              onMouseEnter={() => setActive(i)}
              onClick={() => start(c)}
            >
              <span className="client-badge" aria-hidden="true">
                {c.isNew ? "+" : c.client.name.slice(0, 2).toUpperCase()}
              </span>
              <span className="client-name">{c.isNew ? `Create client "${c.name}"` : c.client.name}</span>
              {!c.isNew && (
                <small>
                  {c.client.name === current
                    ? "current"
                    : `${c.client.sessions.length} session${c.client.sessions.length === 1 ? "" : "s"}`}
                </small>
              )}
            </div>
          ))}
          {!choices.length && <p className="client-empty">Type the client's name to add it.</p>}
        </div>
        {error && (
          <div role="alert" className="dialog-error">
            {error}
          </div>
        )}
        <div className="dialog-actions">
          <span className="dialog-hint">↑↓ to choose · Enter to start</span>
          <button type="button" className="btn" onClick={onCancel}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" disabled={starting || !selected}>
            Start
          </button>
        </div>
      </form>
    </div>
  );
}
