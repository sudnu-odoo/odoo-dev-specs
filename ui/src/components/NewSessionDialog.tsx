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

/** When a client was last worked on: its most recent session. */
const lastUsed = (p: ProjectInfo) => Math.max(0, ...p.sessions.map((s) => s.updated_at));
const optionId = (name: string) => `client-${name}`;
const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

/**
 * Ctrl+N: a new session, for which client? Two clear steps: pick an existing client
 * (searchable, current and recent first, arrows and Enter), or "New client" to type
 * a name. Stays the same size with any number of clients.
 */
export default function NewSessionDialog({ clients, current, onStart, onCancel }: NewSessionDialogProps) {
  const [adding, setAdding] = useState(clients.length === 0);
  const [query, setQuery] = useState("");
  const [name, setName] = useState("");
  const [active, setActive] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const matches = [...clients]
    .filter((c) => words.every((w) => c.name.toLowerCase().includes(w)))
    .sort(
      (a, b) =>
        Number(same(b.name, query)) - Number(same(a.name, query)) ||
        Number(b.name === current) - Number(a.name === current) ||
        lastUsed(b) - lastUsed(a),
    );
  const selected = matches[Math.min(active, matches.length - 1)];

  // Keep the chosen line in view while moving with the arrow keys.
  const selectedName = adding ? null : selected?.name;
  useEffect(() => {
    if (selectedName) document.getElementById(optionId(selectedName))?.scrollIntoView?.({ block: "nearest" });
  }, [selectedName]);

  async function start(client: string, isNew: boolean) {
    if (starting) return;
    setStarting(true);
    try {
      await onStart(client, isNew);
    } catch (err) {
      setError(String(err));
      setStarting(false);
    }
  }

  function submit(e: FormEvent) {
    e.preventDefault();
    if (!adding) {
      if (selected) start(selected.name, false);
      return;
    }
    const typed = name.trim();
    if (!typed) return;
    const existing = clients.find((c) => same(c.name, typed));
    if (existing) setError(`"${existing.name}" already exists: pick it from the list.`);
    else start(typed, true);
  }

  function openAdding(prefill = "") {
    setName(prefill);
    setError(null);
    setAdding(true);
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") onCancel();
    else if (!adding && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      const step = e.key === "ArrowDown" ? 1 : -1;
      setActive((i) => Math.max(0, Math.min(matches.length - 1, Math.min(i, matches.length - 1) + step)));
    }
  }

  return (
    <div
      className="settings-overlay"
      role="dialog"
      aria-label="New session"
      onMouseDown={(e) => e.target === e.currentTarget && onCancel()}
    >
      <form className="dialog new-session" onSubmit={submit} onKeyDown={onKeyDown}>
        {adding ? (
          <>
            <h2>New client</h2>
            <p className="dialog-lead">
              A folder for this client's sessions. Its first session starts right away.
            </p>
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
          </>
        ) : (
          <>
            <div className="dialog-title-row">
              <h2>New session</h2>
              <button type="button" className="btn btn-small" onClick={() => openAdding()}>
                <span aria-hidden="true">+</span> New client
              </button>
            </div>
            <input
              autoFocus
              className="client-search"
              role="combobox"
              aria-label="Client"
              aria-expanded="true"
              aria-controls="client-list"
              aria-activedescendant={selected ? optionId(selected.name) : undefined}
              placeholder="Search clients"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setActive(0);
                setError(null);
              }}
            />
            <div className="client-list" id="client-list" role="listbox" aria-label="Clients">
              {matches.map((c, i) => (
                <div
                  key={c.name}
                  id={optionId(c.name)}
                  role="option"
                  aria-selected={c === selected}
                  className="client-option"
                  onMouseEnter={() => setActive(i)}
                  onClick={() => start(c.name, false)}
                >
                  <span className="client-badge" aria-hidden="true">
                    {c.name.slice(0, 2).toUpperCase()}
                  </span>
                  <span className="client-name">{c.name}</span>
                  <small>
                    {c.name === current
                      ? "current"
                      : `${c.sessions.length} session${c.sessions.length === 1 ? "" : "s"}`}
                  </small>
                </div>
              ))}
              {!matches.length && (
                <div className="client-empty">
                  <p>No client called "{query.trim()}".</p>
                  <button type="button" className="btn" onClick={() => openAdding(query.trim())}>
                    Add "{query.trim()}" as a new client
                  </button>
                </div>
              )}
            </div>
          </>
        )}
        {error && (
          <div role="alert" className="dialog-error">
            {error}
          </div>
        )}
        <div className="dialog-actions">
          {adding && clients.length > 0 ? (
            <button type="button" className="btn dialog-back" onClick={() => setAdding(false)}>
              Back
            </button>
          ) : (
            !adding && <span className="dialog-hint">↑↓ to choose · Enter to start</span>
          )}
          <button type="button" className="btn" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="submit"
            className="btn btn-primary"
            disabled={starting || (adding ? !name.trim() : !selected)}
          >
            {adding ? "Create and start" : "Start"}
          </button>
        </div>
      </form>
    </div>
  );
}
