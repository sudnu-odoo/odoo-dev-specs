import { appsText } from "../lib/odooApps";
import type { DatabaseInfo } from "../lib/types";
import { useState } from "react";
import { usePopover } from "../app/usePopover";

export interface DatabaseMenuProps {
  databases: DatabaseInfo[];
  /** The session's database; null = Auto. */
  selected: string | null;
  disabled?: boolean;
  onChange: (name: string | null) => void;
  onManage: () => void;
  /** Make a new database for this session. */
  onCreate?: () => void;
  /** A database is being made: the chip says so. */
  creating?: boolean;
  /** Open the session's database in the browser. */
  onOpen?: () => void;
  /** It is starting (the Open button waits). */
  opening?: boolean;
}

/** Databases listed before "Show all". */
const SHOWN = 5;

const DbIcon = () => (
  <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
    <ellipse cx="8" cy="3.5" rx="5.5" ry="2" fill="none" stroke="currentColor" strokeWidth="1.3" />
    <path
      d="M2.5 3.5v9c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2v-9M2.5 8c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
    />
  </svg>
);

/**
 * The session's database, as a quiet chip next to the model: "Automatic" until the
 * first screenshots (or the user) pick one. The menu lists the databases given (the
 * session's version), most recently used first: five, then "Show all", and a search
 * once there are more. Automatic stays on top and the actions at the bottom.
 */
export default function DatabaseMenu({
  databases,
  selected,
  disabled,
  onChange,
  onManage,
  onCreate,
  creating = false,
  onOpen,
  opening = false,
}: DatabaseMenuProps) {
  const { open, setOpen, root } = usePopover();

  const current = databases.find((d) => d.name === selected);
  const label = creating
    ? "Creating database…"
    : selected
      ? (current?.label ?? "Deleted database")
      : "Automatic";
  const pick = (name: string | null) => {
    setOpen(false);
    if (name !== selected) onChange(name);
  };
  const [query, setQuery] = useState("");
  const [all, setAll] = useState(false);
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const found = [...databases]
    .sort((a, b) => b.lastUsed - a.lastUsed)
    .filter((d) => {
      const text = `${d.label} ${d.version} ${appsText(d.modules)}`.toLowerCase();
      return words.every((w) => text.includes(w));
    });
  // Searching shows every match; otherwise the five most recent, and the chosen one.
  const shown = all || words.length ? found : found.filter((d, i) => i < SHOWN || d.name === selected);
  const hidden = found.length - shown.length;

  return (
    <div className="model-menu db-chip-group" ref={root}>
      <button
        type="button"
        className="model-chip"
        aria-label={`Database: ${label}`}
        title="Odoo database for this session's screenshots"
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled || creating}
        onClick={() => setOpen((o) => !o)}
      >
        {creating ? <span className="spinner spinner-small" aria-hidden="true" /> : <DbIcon />}
        {label}
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
          <path d="M3 4.5 6 7.5 9 4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </button>
      {onOpen && current && !creating && (
        <button
          type="button"
          className="model-chip db-open"
          aria-label={`Open ${current.label} in the browser`}
          title="Open this database in your browser (admin / admin)"
          disabled={opening}
          onClick={onOpen}
        >
          {opening ? (
            <span className="spinner spinner-small" aria-hidden="true" />
          ) : (
            <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
              <path
                d="M9 3h4v4M13 3 7.5 8.5M11 9.5V13H3V5h3.5"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.4"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          )}
          Open
        </button>
      )}
      {open && (
        <div className="menu menu-up db-menu" role="menu" aria-label="Database">
          <div role="group" className="menu-group">
            <Choice
              checked={!selected}
              onPick={() => pick(null)}
              title="Automatic"
              detail="Any database of the Odoo version; a new one if there is none"
            />
          </div>
          {databases.length > 0 && (
            <div role="group" aria-label="Your databases" className="menu-group">
              <div className="menu-head" aria-hidden="true">
                Your databases
              </div>
              {databases.length > SHOWN && (
                <input
                  autoFocus
                  className="menu-search"
                  aria-label="Search databases"
                  placeholder="Search by name or app…"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && found[0]) pick(found[0].name);
                  }}
                />
              )}
              <div className="menu-scroll">
                {shown.map((d) => (
                  <Choice
                    key={d.name}
                    checked={d.name === selected}
                    onPick={() => pick(d.name)}
                    title={d.label}
                    detail={`${d.version} · ${appsText(d.modules)}`}
                  />
                ))}
                {words.length > 0 && found.length === 0 && <p className="menu-empty">No database matches.</p>}
              </div>
              {hidden > 0 && (
                <button type="button" className="menu-item menu-link" onClick={() => setAll(true)}>
                  Show all ({databases.length})
                </button>
              )}
            </div>
          )}
          <div className="menu-group">
            {onCreate && (
              <button
                type="button"
                role="menuitem"
                className="menu-item menu-link"
                onClick={() => {
                  setOpen(false);
                  onCreate();
                }}
              >
                New database…
              </button>
            )}
            <button
              type="button"
              role="menuitem"
              className="menu-item menu-link"
              onClick={() => {
                setOpen(false);
                onManage();
              }}
            >
              Manage databases…
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function Choice({
  checked,
  onPick,
  title,
  detail,
}: {
  checked: boolean;
  onPick: () => void;
  title: string;
  detail: string;
}) {
  return (
    <div
      role="menuitemradio"
      aria-checked={checked}
      aria-label={title}
      tabIndex={0}
      className="menu-item menu-item-2line"
      onClick={onPick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onPick();
        }
      }}
    >
      <span className="menu-item-text">
        <span>{title}</span>
        <small>{detail}</small>
      </span>
      {checked && (
        <span className="menu-check" aria-hidden="true">
          ✓
        </span>
      )}
    </div>
  );
}
