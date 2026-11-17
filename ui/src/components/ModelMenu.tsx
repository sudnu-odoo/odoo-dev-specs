import { useEffect, useRef, useState } from "react";
import type { Model } from "../lib/types";

interface ModelMenuProps {
  models: Model[];
  selected: string;
  disabled?: boolean;
  onChange?: (id: string) => void;
}

/** Group models by provider, keeping first-seen order; "" holds models with no provider. */
function groupByProvider(models: Model[]): [string, Model[]][] {
  const groups = new Map<string, Model[]>();
  for (const m of models) {
    const key = m.provider ?? "";
    groups.set(key, [...(groups.get(key) ?? []), m]);
  }
  return [...groups];
}

/**
 * A quiet chip showing the chosen model; it opens a small menu above the composer
 * with one section per provider. Providers that are not ready are shown, locked.
 */
export default function ModelMenu({ models, selected, disabled, onChange }: ModelMenuProps) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const current = models.find((m) => m.id === selected);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  function pick(m: Model) {
    if (m.locked) return;
    onChange?.(m.id);
    setOpen(false);
  }

  return (
    <div className="model-menu" ref={root}>
      <button
        type="button"
        className="model-chip"
        aria-label={`Model: ${current?.label ?? "default"}`}
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        {current?.label ?? "Default model"}
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
          <path d="M3 4.5 6 7.5 9 4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </button>
      {open && (
        <div className="menu menu-up" role="menu" aria-label="Model">
          {groupByProvider(models).map(([provider, group]) => {
            const locked = group.every((m) => m.locked);
            const name = provider ? (locked ? `${provider} (not ready)` : provider) : "Models";
            return (
              <div key={provider} role="group" aria-label={name} className="menu-group">
                <div className="menu-head" aria-hidden="true">
                  {provider || "Models"}
                  {locked && (
                    <span className="menu-lock" title="Not installed or not logged in">
                      🔒 log in to use
                    </span>
                  )}
                </div>
                {group.map((m) => (
                  <div
                    key={m.id}
                    role="menuitemradio"
                    aria-checked={m.id === selected}
                    aria-disabled={m.locked || undefined}
                    tabIndex={m.locked ? -1 : 0}
                    className="menu-item"
                    onClick={() => pick(m)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        pick(m);
                      }
                    }}
                  >
                    <span>{m.label}</span>
                    {m.id === selected && (
                      <span className="menu-check" aria-hidden="true">
                        ✓
                      </span>
                    )}
                  </div>
                ))}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
