import { useRef, useState, type FormEvent } from "react";
import type { ProjectInfo, SessionRef } from "../lib/types";

export interface SidebarProps {
  projects: ProjectInfo[];
  selected: SessionRef | null;
  onSelect: (ref: SessionRef) => void;
  /** Rejects with a user-facing message when the name is not accepted. */
  onCreateProject: (name: string) => Promise<void>;
  onCreateSession: (project: string) => void;
  onDeleteSession: (ref: SessionRef) => void;
  onRenameSession: (ref: SessionRef, title: string) => Promise<void>;
  onOpenSettings: () => void;
  /** The Databases page is showing (instead of a session). */
  databasesOpen?: boolean;
  onOpenDatabases?: () => void;
  /** A setup download still going on; clicking it opens the setup. */
  download?: { label: string; percent: number | null } | null;
  onOpenSetup?: () => void;
}

export default function Sidebar({
  projects,
  selected,
  onSelect,
  onCreateProject,
  onCreateSession,
  onDeleteSession,
  onRenameSession,
  onOpenSettings,
  databasesOpen = false,
  onOpenDatabases,
  download,
  onOpenSetup,
}: SidebarProps) {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="sidebar-title">
          <span className="brand">Odoo Dev Specs</span>
          <button className="icon-btn" aria-label="Settings" title="Settings" onClick={onOpenSettings}>
            ⚙
          </button>
        </div>
        <NewProjectForm onCreate={onCreateProject} />
      </div>
      {onOpenDatabases && (
        <button
          className={"sidebar-nav" + (databasesOpen ? " active" : "")}
          aria-current={databasesOpen ? "page" : undefined}
          onClick={onOpenDatabases}
        >
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <ellipse cx="8" cy="3.5" rx="5.5" ry="2" fill="none" stroke="currentColor" strokeWidth="1.3" />
            <path
              d="M2.5 3.5v9c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2v-9M2.5 8c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.3"
            />
          </svg>
          Databases
        </button>
      )}
      <nav className="tree" aria-label="Projects">
        {projects.length === 0 && (
          <p className="tree-empty">Create a project for your customer, then start a session in it.</p>
        )}
        {projects.map((p) => (
          <div key={p.name} className="tree-project">
            <div className="tree-project-header">
              <span className="tree-project-name">{p.name}</span>
              <button
                className="icon-btn"
                aria-label={`New session in ${p.name}`}
                title="New session"
                onClick={() => onCreateSession(p.name)}
              >
                +
              </button>
            </div>
            {p.sessions.map((s) => {
              const ref = { project: p.name, sessionId: s.id };
              return (
                <SessionRow
                  key={s.id}
                  title={s.title}
                  active={!databasesOpen && selected?.project === p.name && selected.sessionId === s.id}
                  onSelect={() => onSelect(ref)}
                  onRename={(title) => onRenameSession(ref, title)}
                  onDelete={() => onDeleteSession(ref)}
                />
              );
            })}
          </div>
        ))}
      </nav>
      {download && (
        <button className="sidebar-download" onClick={onOpenSetup} title="Open the setup">
          <svg className="download-ring" viewBox="0 0 16 16" aria-hidden="true">
            <circle cx="8" cy="8" r="6.5" fill="none" stroke="var(--border)" strokeWidth="2" />
            <circle
              cx="8"
              cy="8"
              r="6.5"
              fill="none"
              stroke="var(--accent)"
              strokeWidth="2"
              strokeLinecap="round"
              strokeDasharray={`${((download.percent ?? 25) / 100) * 40.8} 40.8`}
              transform="rotate(-90 8 8)"
            />
          </svg>
          <span role="status">
            {download.label}
            {download.percent !== null ? ` · ${download.percent}%` : "…"}
          </span>
        </button>
      )}
    </aside>
  );
}

interface SessionRowProps {
  title: string;
  active: boolean;
  onSelect: () => void;
  onRename: (title: string) => Promise<void>;
  onDelete: () => void;
}

function SessionRow({ title, active, onSelect, onRename, onDelete }: SessionRowProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(title);
  // Enter closes the field, which may also fire blur: finish each edit only once.
  const finished = useRef(false);

  function startEditing() {
    finished.current = false;
    setDraft(title);
    setEditing(true);
  }

  function cancel() {
    finished.current = true;
    setEditing(false);
  }

  function commit() {
    if (finished.current) return;
    finished.current = true;
    setEditing(false);
    const next = draft.trim();
    if (next && next !== title) onRename(next).catch(() => {});
  }

  if (editing) {
    return (
      <div className={"tree-session" + (active ? " active" : "")}>
        <input
          className="tree-session-input"
          aria-label="Session name"
          autoFocus
          value={draft}
          onFocus={(e) => e.target.select()}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            if (e.key === "Escape") cancel();
          }}
        />
      </div>
    );
  }
  return (
    <div className={"tree-session" + (active ? " active" : "")}>
      <button
        className="tree-session-title"
        aria-current={active ? "true" : undefined}
        onClick={onSelect}
        onDoubleClick={startEditing}
      >
        {title}
      </button>
      <button
        className="icon-btn tree-session-action"
        aria-label={`Rename ${title}`}
        title="Rename"
        onClick={startEditing}
      >
        ✎
      </button>
      <button
        className="icon-btn tree-session-action"
        aria-label={`Delete ${title}`}
        title="Delete"
        onClick={onDelete}
      >
        ×
      </button>
    </div>
  );
}

function NewProjectForm({ onCreate }: { onCreate: (name: string) => Promise<void> }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    try {
      await onCreate(name);
      setOpen(false);
      setName("");
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }

  if (!open) {
    return (
      <button className="btn btn-primary btn-small" onClick={() => setOpen(true)}>
        New project
      </button>
    );
  }
  return (
    <form className="new-project" onSubmit={submit}>
      <input
        autoFocus
        placeholder="Project name"
        value={name}
        onChange={(e) => setName(e.target.value)}
        onKeyDown={(e) => e.key === "Escape" && setOpen(false)}
      />
      {error && (
        <p role="alert" className="form-error">
          {error}
        </p>
      )}
    </form>
  );
}
