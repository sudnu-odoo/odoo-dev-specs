import { useEffect, useRef, useState } from "react";
import type { Hosting } from "../lib/types";

const CHOICES: { value: Hosting; title: string; detail: string }[] = [
  {
    value: "online",
    title: "Odoo Online",
    detail: "No custom Python code: the assistant checks what is possible first",
  },
  { value: "sh", title: "Odoo.sh", detail: "Custom modules are possible" },
];

export interface HostingMenuProps {
  /** null until chosen: the assistant asks. */
  hosting: Hosting | null;
  disabled?: boolean;
  onChange: (hosting: Hosting) => void;
}

/** Where the client's Odoo runs, as a chip next to the model; it shapes what the assistant proposes. */
export default function HostingMenu({ hosting, disabled, onChange }: HostingMenuProps) {
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
  const current = CHOICES.find((c) => c.value === hosting);
  const label = current?.title ?? "Hosting?";
  return (
    <div className="model-menu" ref={root}>
      <button
        type="button"
        className={"model-chip" + (current ? "" : " chip-attention")}
        aria-label={`Hosting: ${current?.title ?? "not chosen"}`}
        title="Where the client's Odoo runs"
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        {label}
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
          <path d="M3 4.5 6 7.5 9 4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </button>
      {open && (
        <div className="menu menu-up db-menu" role="menu" aria-label="Hosting">
          <div className="menu-head" aria-hidden="true">
            Where the client's Odoo runs
          </div>
          {CHOICES.map((c) => (
            <div
              key={c.value}
              role="menuitemradio"
              aria-checked={c.value === hosting}
              aria-label={c.title}
              tabIndex={0}
              className="menu-item menu-item-2line"
              onClick={() => {
                setOpen(false);
                if (c.value !== hosting) onChange(c.value);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  setOpen(false);
                  onChange(c.value);
                }
              }}
            >
              <span className="menu-item-text">
                <span>{c.title}</span>
                <small>{c.detail}</small>
              </span>
              {c.value === hosting && (
                <span className="menu-check" aria-hidden="true">
                  ✓
                </span>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
