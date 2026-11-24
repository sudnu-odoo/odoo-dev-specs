import { useRef, useState, type DragEvent, type ReactNode } from "react";
import { useClosedProjects } from "../app/useClosedProjects";
import type { ProjectInfo, SessionMeta, SessionRef } from "../lib/types";
import { versionLabel } from "./HostingMenu";
import { Icon, type IconName } from "./Icon";

export interface SidebarProps {
  /** Projects; the sessions without a project come as the one named "". */
  projects: ProjectInfo[];
  selected: SessionRef | null;
  onSelect: (ref: SessionRef) => void;
  /** Start a session without a project (Ctrl+N). */
  onNewSession: () => void;
  /** Open the search (Ctrl+K). */
  onSearch: () => void;
  /** Ask for a new project's name. */
  onNewProject: () => void;
  onCreateSession: (project: string) => void;
  onDeleteSession: (ref: SessionRef) => void;
  /** Delete a project and all its sessions (asks first). */
  onDeleteProject: (project: string) => void;
  /** Put a session in a project ("" = none), by drag and drop. */
  onMoveSession: (ref: SessionRef, project: string) => void;
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
  onNewProject,
  onCreateSession,
  onDeleteSession,
  onDeleteProject,
  onMoveSession,
  onRenameSession,
  onOpenSettings,
  databasesOpen = false,
  onOpenDatabases,
  download,
  onOpenSetup,
  running = [],
}: SidebarProps) {
  const closed = useClosedProjects();
  // The session being dragged, and the place it would land ("" = no project).
  const [dragging, setDragging] = useState<SessionRef | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const loose = projects.find((p) => p.name === "")?.sessions ?? [];
  const named = projects.filter((p) => p.name !== "");

  /** Props that make an element a place to drop a session into `project`. */
  const dropInto = (project: string) => ({
    "data-drop": dragging && dragging.project !== project && over === project ? "true" : undefined,
    onDragOver: (e: DragEvent) => {
      if (!dragging || dragging.project === project) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      setOver(project);
    },
    onDragLeave: (e: DragEvent) => {
      if (!e.currentTarget.contains(e.relatedTarget as Node | null))
        setOver((o) => (o === project ? null : o));
    },
    onDrop: (e: DragEvent) => {
      e.preventDefault();
      if (dragging && dragging.project !== project) {
        if (project) closed.open(project);
        onMoveSession(dragging, project);
      }
      setDragging(null);
      setOver(null);
    },
  });

  const row = (s: SessionMeta, project: string, nested: boolean) => {
    const ref = { project, sessionId: s.id };
    return (
      <SessionRow
        key={s.id}
        title={s.title}
        meta={sessionMeta(s)}
        when={ago(s.updated_at)}
        nested={nested}
        working={running.includes(s.id)}
        active={!databasesOpen && selected?.sessionId === s.id}
        onSelect={() => onSelect(ref)}
        onRename={(title) => onRenameSession(ref, title)}
        onDelete={() => onDeleteSession(ref)}
        onDragStart={() => setDragging(ref)}
        onDragEnd={() => {
          setDragging(null);
          setOver(null);
        }}
      />
    );
  };

  return (
    <aside className="sidebar">
      <div className="sidebar-title">
        <span className="brand">Odoo Dev Specs</span>
        <button className="icon-btn" aria-label="Settings" title="Settings" onClick={onOpenSettings}>
          <Icon name="settings" />
        </button>
      </div>
      <div className="side-nav">
        <NavRow icon="compose" label="New session" shortcut="Ctrl N" onClick={onNewSession} />
        <NavRow icon="search" label="Search" shortcut="Ctrl K" onClick={onSearch} />
        {onOpenDatabases && (
          <NavRow icon="database" label="Databases" active={databasesOpen} onClick={onOpenDatabases} />
        )}
      </div>
      <nav className="tree" aria-label="Projects">
        <SectionTitle title="Projects">
          <button className="icon-btn" aria-label="New project" title="New project" onClick={onNewProject}>
            <Icon name="plus" />
          </button>
        </SectionTitle>
        {named.length === 0 && <p className="tree-empty">Group a client's sessions in a project.</p>}
        {named.map((p) => {
          const open = !closed.has(p.name);
          return (
            <div key={p.name} className="tree-project" {...dropInto(p.name)}>
              <div className="side-row tree-project-header">
                <button
                  className="side-row-main"
                  aria-expanded={open}
                  title={open ? "Hide sessions" : "Show sessions"}
                  onClick={() => closed.toggle(p.name)}
                >
                  <Icon name={open ? "folder-open" : "folder"} />
                  <span className="side-row-label">{p.name}</span>
                </button>
                <button
                  className="icon-btn side-row-action"
                  aria-label={`Delete project ${p.name}`}
                  title="Delete project"
                  onClick={() => onDeleteProject(p.name)}
                >
                  <Icon name="trash" />
                </button>
                <button
                  className="icon-btn side-row-action"
                  aria-label={`New session in ${p.name}`}
                  title="New session in this project"
                  onClick={() => {
                    closed.open(p.name);
                    onCreateSession(p.name);
                  }}
                >
                  <Icon name="plus" />
                </button>
              </div>
              {open && p.sessions.map((s) => row(s, p.name, true))}
              {open && p.sessions.length === 0 && <p className="tree-empty nested">No sessions yet.</p>}
            </div>
          );
        })}
        <div className="tree-loose" {...dropInto("")}>
          <SectionTitle title="Sessions" />
          {loose.map((s) => row(s, "", false))}
          {loose.length === 0 &&
            (dragging?.project ? (
              <p className="tree-empty">Drop here to take it out of its project.</p>
            ) : named.length === 0 ? (
              <p className="tree-empty">Start with New session, then describe what the client needs.</p>
            ) : null)}
        </div>
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

function NavRow(props: {
  icon: IconName;
  label: string;
  shortcut?: string;
  active?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={"side-row side-row-main" + (props.active ? " active" : "")}
      aria-label={props.shortcut ? `${props.label} ${props.shortcut}` : props.label}
      aria-current={props.active ? "page" : undefined}
      onClick={props.onClick}
    >
      <Icon name={props.icon} />
      <span className="side-row-label">{props.label}</span>
      {props.shortcut && <kbd>{props.shortcut}</kbd>}
    </button>
  );
}

function SectionTitle({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="side-section">
      <span>{title}</span>
      {children}
    </div>
  );
}

interface SessionRowProps {
  title: string;
  /** "Online · 19.2 · 2h": the client's hosting and version, and when last used. */
  meta: string;
  /** How long ago it was used: "2h". */
  when: string | null;
  /** Inside a project: indented under its name. */
  nested: boolean;
  working: boolean;
  active: boolean;
  onSelect: () => void;
  onRename: (title: string) => Promise<void>;
  onDelete: () => void;
  onDragStart: () => void;
  onDragEnd: () => void;
}

function SessionRow(p: SessionRowProps) {
  const { title, meta, when, nested, working, active, onSelect, onRename, onDelete } = p;
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(title);
  // Enter closes the field, which may also fire blur: finish each edit only once.
  const finished = useRef(false);
  const className = "side-row tree-session" + (nested ? " nested" : "") + (active ? " active" : "");

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
      <div className={className}>
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
    <div
      className={className}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", title);
        p.onDragStart();
      }}
      onDragEnd={p.onDragEnd}
    >
      <button
        className="side-row-main"
        aria-label={title}
        aria-current={active ? "true" : undefined}
        title={meta ? `${title}\n${meta}` : title}
        onClick={onSelect}
        onDoubleClick={startEditing}
      >
        {!nested && <Icon name="chat" />}
        <span className="side-row-label">{title}</span>
      </button>
      {working ? (
        <span className="spinner spinner-small" role="status" aria-label="Working" />
      ) : (
        when && <small className="side-row-hint">{when}</small>
      )}
      <button
        className="icon-btn side-row-action"
        aria-label={`Rename ${title}`}
        title="Rename"
        onClick={startEditing}
      >
        <Icon name="pencil" />
      </button>
      <button
        className="icon-btn side-row-action"
        aria-label={`Delete ${title}`}
        title="Delete"
        onClick={onDelete}
      >
        <Icon name="trash" />
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
