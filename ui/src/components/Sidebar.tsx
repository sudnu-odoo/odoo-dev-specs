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
                  active={selected?.project === p.name && selected.sessionId === s.id}
                  onSelect={() => onSelect(ref)}
                  onRename={(title) => onRenameSession(ref, title)}
                  onDelete={() => onDeleteSession(ref)}
                />
              );
            })}
          </div>
        ))}
      </nav>
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
