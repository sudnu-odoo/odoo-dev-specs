import { useState, type FormEvent } from "react";
import type { ProjectInfo } from "../lib/types";

export interface NewSessionDialogProps {
  clients: ProjectInfo[];
  /** `isNew`: the client is created first. Rejects with a message for the person. */
  onStart: (client: string, isNew: boolean) => Promise<void>;
  onCancel: () => void;
}

const NEW = "\u0000new";

/** When a client was last worked on: its most recent session. */
const lastUsed = (p: ProjectInfo) => Math.max(0, ...p.sessions.map((s) => s.updated_at));

/**
 * Ctrl+N: a new session, for which client? The clients most recently worked on come
 * first; "New client" adds one by name.
 */
export default function NewSessionDialog({ clients, onStart, onCancel }: NewSessionDialogProps) {
  const sorted = [...clients].sort((a, b) => lastUsed(b) - lastUsed(a));
  const [chosen, setChosen] = useState(sorted[0]?.name ?? NEW);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const isNew = chosen === NEW;

  async function start(e?: FormEvent) {
    e?.preventDefault();
    const client = isNew ? name.trim() : chosen;
    if (!client || starting) return;
    setStarting(true);
    try {
      await onStart(client, isNew);
    } catch (err) {
      setError(String(err));
      setStarting(false);
    }
  }

  return (
    <div
      className="settings-overlay"
      role="dialog"
      aria-label="New session"
      onMouseDown={(e) => e.target === e.currentTarget && onCancel()}
    >
      <form
        className="dialog new-session"
        onSubmit={start}
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
        }}
      >
        <h2>New session</h2>
        <p className="dialog-lead">For which client?</p>
        <div className="client-list" role="listbox" aria-label="Clients">
          {sorted.map((c) => (
            <div
              key={c.name}
              role="option"
              aria-selected={chosen === c.name}
              tabIndex={0}
              className="client-option"
              onClick={() => setChosen(c.name)}
              onDoubleClick={() => onStart(c.name, false).catch((err) => setError(String(err)))}
              onKeyDown={(e) => e.key === " " && setChosen(c.name)}
            >
              <span className="client-badge" aria-hidden="true">
                {c.name.slice(0, 2).toUpperCase()}
              </span>
              <span className="client-name">{c.name}</span>
              <small>
                {c.sessions.length} session{c.sessions.length === 1 ? "" : "s"}
              </small>
            </div>
          ))}
          <div
            role="option"
            aria-selected={isNew}
            tabIndex={0}
            className="client-option client-new"
            onClick={() => setChosen(NEW)}
            onKeyDown={(e) => e.key === " " && setChosen(NEW)}
          >
            <span className="client-badge" aria-hidden="true">
              +
            </span>
            <span className="client-name">New client</span>
          </div>
        </div>
        {isNew && (
          <label className="dialog-field">
            <span>Client name</span>
            <input
              autoFocus
              aria-label="Client name"
              placeholder="e.g. Deco Addict"
              value={name}
              onChange={(e) => {
                setName(e.target.value);
                setError(null);
              }}
            />
          </label>
        )}
        {error && (
          <div role="alert" className="dialog-error">
            {error}
          </div>
        )}
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onCancel}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" disabled={starting || (isNew && !name.trim())}>
            Start
          </button>
        </div>
      </form>
    </div>
  );
}
