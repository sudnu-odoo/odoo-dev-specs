import type { ReactNode } from "react";

export interface SessionHeaderProps {
  project: string;
  title: string;
  specOpen: boolean;
  onToggleSpec: () => void;
  /** More buttons on the right (e.g. open the spec in its own window). */
  actions?: ReactNode;
}

/** The open session's title bar: "Client / Session", and the button showing the spec. */
export default function SessionHeader({
  project,
  title,
  specOpen,
  onToggleSpec,
  actions,
}: SessionHeaderProps) {
  return (
    <header className="session-header">
      <h1 className="session-crumbs" aria-label={`${project} / ${title}`}>
        <span className="session-crumb-project">{project}</span>
        <span className="session-crumb-sep" aria-hidden="true">
          /
        </span>
        <span className="session-crumb-title">{title}</span>
      </h1>
      <div className="session-header-actions">
        {actions}
        <button
          className="btn btn-small spec-toggle"
          aria-pressed={specOpen}
          title={`${specOpen ? "Hide" : "Show"} the spec (Ctrl+B)`}
          onClick={onToggleSpec}
        >
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <rect
              x="1.5"
              y="2.5"
              width="13"
              height="11"
              rx="2"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.3"
            />
            <path d="M9.5 2.5v11" stroke="currentColor" strokeWidth="1.3" />
          </svg>
          Spec
          <kbd>Ctrl B</kbd>
        </button>
      </div>
    </header>
  );
}
