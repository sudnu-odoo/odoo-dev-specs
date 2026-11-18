import { useEffect, useRef, useState } from "react";
import { appsText } from "../lib/odooApps";
import type { DatabaseInfo } from "../lib/types";

export interface DatabaseMenuProps {
  databases: DatabaseInfo[];
  /** The session's database; null = Auto. */
  selected: string | null;
  disabled?: boolean;
  onChange: (name: string | null) => void;
  onManage: () => void;
}

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
 * The session's database, as a quiet chip next to the model: "Auto" until the
 * first screenshots (or the user) pick one. The menu lists every database.
 */
export default function DatabaseMenu({
  databases,
  selected,
  disabled,
  onChange,
  onManage,
}: DatabaseMenuProps) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !root.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  const current = databases.find((d) => d.name === selected);
  const label = selected ? (current?.label ?? "Deleted database") : "Auto";
  const pick = (name: string | null) => {
    setOpen(false);
    if (name !== selected) onChange(name);
  };
  const groups: [string, DatabaseInfo[]][] = [
    ["Your databases", databases.filter((d) => !d.auto)],
    ["Made by the assistant", databases.filter((d) => d.auto)],
  ];

  return (
    <div className="model-menu" ref={root}>
      <button
        type="button"
        className="model-chip"
        aria-label={`Database: ${label}`}
        title="Odoo database for this session's screenshots"
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        <DbIcon />
        {label}
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
          <path d="M3 4.5 6 7.5 9 4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </button>
      {open && (
        <div className="menu menu-up db-menu" role="menu" aria-label="Database">
          <div role="group" className="menu-group">
            <Choice
              checked={!selected}
              onPick={() => pick(null)}
              title="Auto"
              detail="A fresh demo database for the feature"
            />
          </div>
          {groups
            .filter(([, list]) => list.length > 0)
            .map(([name, list]) => (
              <div key={name} role="group" aria-label={name} className="menu-group">
                <div className="menu-head" aria-hidden="true">
                  {name}
                </div>
                {list.map((d) => (
                  <Choice
                    key={d.name}
                    checked={d.name === selected}
                    onPick={() => pick(d.name)}
                    title={d.label}
                    detail={`${d.version} · ${appsText(d.modules)}`}
                  />
                ))}
              </div>
            ))}
          <div className="menu-group">
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
