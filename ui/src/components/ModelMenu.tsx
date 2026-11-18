import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Model } from "../lib/types";

/** Effort levels both CLIs accept; "" leaves the CLI's default. */
export const EFFORTS = ["", "low", "medium", "high", "xhigh", "max"] as const;

const effortLabel = (e: string) => (e ? e[0].toUpperCase() + e.slice(1) : "Default");

interface ModelMenuProps {
  models: Model[];
  selected: string;
  effort?: string;
  disabled?: boolean;
  onChange?: (id: string) => void;
  onEffortChange?: (effort: string) => void;
  /** Start the login of a provider (its lower-case id, e.g. "antigravity"). */
  onLogin?: (provider: string) => void;
  /** The provider this session is tied to (e.g. "Claude"); others are unavailable. */
  sessionProvider?: string;
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

function Item({
  checked,
  locked,
  reason = "Log in to use this model",
  onPick,
  children,
}: {
  checked: boolean;
  locked?: boolean;
  reason?: string;
  onPick: () => void;
  children: ReactNode;
}) {
  return (
    <div
      role="menuitemradio"
      aria-checked={checked}
      aria-disabled={locked || undefined}
      tabIndex={locked ? -1 : 0}
      className="menu-item"
      title={locked ? reason : undefined}
      onClick={() => !locked && onPick()}
      onKeyDown={(e) => {
        if (!locked && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          onPick();
        }
      }}
    >
      {children}
      {locked && (
        <span className="menu-lock-icon" aria-hidden="true">
          🔒
        </span>
      )}
      {checked && (
        <span className="menu-check" aria-hidden="true">
          ✓
        </span>
      )}
    </div>
  );
}

/**
 * A quiet chip showing the chosen model and effort; it opens a menu above the composer
 * with one section per provider, then the effort levels. A provider that is not ready
 * has its models locked and a button to log in.
 */
export default function ModelMenu({
  models,
  selected,
  effort = "",
  disabled,
  onChange,
  onEffortChange,
  onLogin,
  sessionProvider,
}: ModelMenuProps) {
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

  const label = current?.label ?? "Default model";
  return (
    <div className="model-menu" ref={root}>
      <button
        type="button"
        className="model-chip"
        aria-label={`Model: ${label}${effort ? `, ${effortLabel(effort)} effort` : ""}`}
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        {label}
        {effort && <span className="model-chip-effort">· {effortLabel(effort)}</span>}
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
          <path d="M3 4.5 6 7.5 9 4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </button>
      {open && (
        <div className="menu menu-up" role="menu" aria-label="Model and effort">
          {groupByProvider(models).map(([provider, group]) => {
            // A started session stays with its provider: the others are not a login matter.
            const otherProvider = !!sessionProvider && !!provider && provider !== sessionProvider;
            const locked = group.every((m) => m.locked);
            const name = otherProvider
              ? `${provider} (this session uses ${sessionProvider})`
              : provider
                ? locked
                  ? `${provider} (not ready)`
                  : provider
                : "Models";
            return (
              <div key={provider} role="group" aria-label={name} className="menu-group">
                <div className="menu-head">
                  <span aria-hidden="true">{provider || "Models"}</span>
                  {otherProvider && <span className="menu-note">new session to use</span>}
                  {locked && provider && !otherProvider && (
                    <button
                      type="button"
                      className="menu-login"
                      aria-label={`Log in to ${provider}`}
                      onClick={() => {
                        onLogin?.(provider.toLowerCase());
                        setOpen(false);
                      }}
                    >
                      Log in
                    </button>
                  )}
                </div>
                {group.map((m) => (
                  <Item
                    key={m.id}
                    checked={m.id === selected}
                    locked={m.locked || otherProvider}
                    reason={
                      otherProvider
                        ? `This session uses ${sessionProvider}; start a new session to use ${provider}`
                        : undefined
                    }
                    onPick={() => {
                      onChange?.(m.id);
                      setOpen(false);
                    }}
                  >
                    <span>{m.label}</span>
                  </Item>
                ))}
              </div>
            );
          })}
          {onEffortChange && (
            <div role="group" aria-label="Effort" className="menu-group">
              <div className="menu-head" aria-hidden="true">
                Effort
              </div>
              {EFFORTS.map((e) => (
                <Item key={e || "default"} checked={e === effort} onPick={() => onEffortChange(e)}>
                  <span>{effortLabel(e)}</span>
                </Item>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
