import { useState, type FormEvent } from "react";
import type { ProjectInfo, SessionRef } from "../lib/types";

export interface SidebarProps {
  projects: ProjectInfo[];
  selected: SessionRef | null;
  onSelect: (ref: SessionRef) => void;
  /** Rejects with a user-facing message when the name is not accepted. */
  onCreateProject: (name: string) => Promise<void>;
  onCreateSession: (project: string) => void;
  onDeleteSession: (ref: SessionRef) => void;
}

export default function Sidebar({
  projects,
  selected,
  onSelect,
  onCreateProject,
  onCreateSession,
  onDeleteSession,
}: SidebarProps) {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <span className="brand">Odoo Dev Specs</span>
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
              const active = selected?.project === p.name && selected.sessionId === s.id;
              return (
                <div key={s.id} className={"tree-session" + (active ? " active" : "")}>
                  <button
                    className="tree-session-title"
                    aria-current={active ? "true" : undefined}
                    onClick={() => onSelect(ref)}
                  >
                    {s.title}
                  </button>
                  <button
                    className="icon-btn tree-session-delete"
                    aria-label={`Delete ${s.title}`}
                    title="Delete session"
                    onClick={() => onDeleteSession(ref)}
                  >
                    ×
                  </button>
                </div>
              );
            })}
          </div>
        ))}
      </nav>
    </aside>
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
