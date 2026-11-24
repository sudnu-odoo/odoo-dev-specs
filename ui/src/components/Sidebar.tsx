import { useRef, useState } from "react";
import { useClosedProjects } from "../app/useClosedProjects";
import type { ProjectInfo, SessionMeta, SessionRef } from "../lib/types";
import { versionLabel } from "./HostingMenu";

export interface SidebarProps {
  projects: ProjectInfo[];
  selected: SessionRef | null;
  onSelect: (ref: SessionRef) => void;
  /** Open the New session dialog (Ctrl+N). */
  onNewSession: () => void;
  /** Open the search (Ctrl+K). */
  onSearch: () => void;
  onCreateSession: (project: string) => void;
  onDeleteSession: (ref: SessionRef) => void;
  /** Delete a client and all its sessions (asks first). */
  onDeleteProject: (project: string) => void;
  onRenameSession: (ref: SessionRef, title: string) => Promise<void>;
  onOpenSettings: () => void;
  /** The Databases page is showing (instead of a session). */
  databasesOpen?: boolean;
  onOpenDatabases?: () => void;
  /** Ids of the sessions whose assistant is answering right now. */
  running?: string[];
  /** A setup download still going on; clicking it opens the setup. */
  download?: { label: string; percent: number | null } | null;
  onOpenSetup?: () => void;
}

export default function Sidebar({
  projects,
  selected,
  onSelect,
  onNewSession,
  onSearch,
  onCreateSession,
  onDeleteSession,
  onDeleteProject,
  onRenameSession,
  onOpenSettings,
  databasesOpen = false,
  onOpenDatabases,
  download,
  onOpenSetup,
  running = [],
}: SidebarProps) {
  const closed = useClosedProjects();
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="sidebar-title">
          <span className="brand">Odoo Dev Specs</span>
          <button className="icon-btn" aria-label="Settings" title="Settings" onClick={onOpenSettings}>
            ⚙
          </button>
        </div>
        <button className="btn btn-primary sidebar-new" onClick={onNewSession}>
          <span>New session</span>
          <kbd>Ctrl N</kbd>
        </button>
        <button className="sidebar-search" onClick={onSearch}>
          <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
            <circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" strokeWidth="1.4" />
            <path d="m10.5 10.5 3 3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
          </svg>
          <span>Search</span>
          <kbd>Ctrl K</kbd>
        </button>
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
          <p className="tree-empty">
            Start with New session: pick or add the client, then describe what they need.
          </p>
        )}
        {projects.map((p) => (
          <div key={p.name} className="tree-project">
            <div className="tree-project-header">
              <button
                className="tree-project-name"
                aria-expanded={!closed.has(p.name)}
                title={closed.has(p.name) ? "Show sessions" : "Hide sessions"}
                onClick={() => closed.toggle(p.name)}
              >
                <svg
                  className="tree-chevron"
                  viewBox="0 0 12 12"
                  width="10"
                  height="10"
                  aria-hidden="true"
                  data-open={!closed.has(p.name)}
                >
                  <path d="M4.5 3 7.5 6 4.5 9" fill="none" stroke="currentColor" strokeWidth="1.5" />
                </svg>
                {p.name}
              </button>
              <button
                className="icon-btn tree-project-action"
                aria-label={`Delete client ${p.name}`}
                title="Delete client"
                onClick={() => onDeleteProject(p.name)}
              >
                ×
              </button>
              <button
                className="icon-btn"
                aria-label={`New session in ${p.name}`}
                title="New session"
                onClick={() => {
                  closed.open(p.name);
                  onCreateSession(p.name);
                }}
              >
                +
              </button>
            </div>
            {!closed.has(p.name) &&
              p.sessions.map((s) => {
                const ref = { project: p.name, sessionId: s.id };
                return (
                  <SessionRow
                    key={s.id}
                    title={s.title}
                    meta={sessionMeta(s)}
                    working={running.includes(s.id)}
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
  /** "Online · 19.2 · 2h": the client's hosting and version, and when last used. */
  meta: string;
  working: boolean;
  active: boolean;
  onSelect: () => void;
  onRename: (title: string) => Promise<void>;
  onDelete: () => void;
}

function SessionRow({ title, meta, working, active, onSelect, onRename, onDelete }: SessionRowProps) {
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
        aria-label={title}
        aria-current={active ? "true" : undefined}
        onClick={onSelect}
        onDoubleClick={startEditing}
      >
        <span className="tree-session-name">{title}</span>
        {(meta || working) && (
          <span className="tree-session-meta">
            {working && <span className="spinner spinner-small" role="status" aria-label="Working" />}
            {meta}
          </span>
        )}
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

/** "Online · 19.2 · 2h": hosting and version when chosen, and how long ago it was used. */
function sessionMeta(s: SessionMeta): string {
  const hosting = s.hosting === "online" ? "Online" : s.hosting === "sh" ? "Odoo.sh" : null;
  return [hosting, s.version ? versionLabel(s.version) : null, ago(s.updated_at)].filter(Boolean).join(" · ");
}

/** "now", "5m", "2h", "3d" since a time in ms; nothing when unknown. */
function ago(ms: number): string | null {
  if (!ms) return null;
  const minutes = Math.floor((Date.now() - ms) / 60_000);
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m`;
  if (minutes < 24 * 60) return `${Math.floor(minutes / 60)}h`;
  return `${Math.floor(minutes / (24 * 60))}d`;
}
