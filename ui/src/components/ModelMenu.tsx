import { useState, type ReactNode } from "react";
import type { Model } from "../lib/types";
import { usePopover } from "../app/usePopover";

/** Effort levels both CLIs accept; "" leaves the CLI's default. */
export const EFFORTS = ["", "low", "medium", "high", "xhigh", "max"] as const;

const effortLabel = (e: string) => (e ? e[0].toUpperCase() + e.slice(1) : "Default");

/** Models like "gemini-3.8-flash-low" carry their level in the id: no separate effort. */
export const hasBuiltInLevel = (id: string) => /-(low|medium|high)$/.test(id);

/**
 * The name on the chip: "Sonnet (balanced)" -> "Sonnet"; a built-in level stays,
 * quietly: "Gemini 3.8 Flash (Low)" -> ["Gemini 3.8 Flash", "Low"].
 */
export function chipName(model: Model): [string, string] {
  const match = /^(.*?)\s*\(([^)]*)\)$/.exec(model.label);
  if (!match) return [model.label, ""];
  return [match[1], hasBuiltInLevel(model.id) ? match[2] : ""];
}

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
  disabled,
  title,
  onPick,
  className = "menu-item",
  children,
}: {
  checked: boolean;
  disabled?: boolean;
  title?: string;
  onPick: () => void;
  className?: string;
  children: ReactNode;
}) {
  return (
    <div
      role="menuitemradio"
      aria-checked={checked}
      aria-disabled={disabled || undefined}
      tabIndex={disabled ? -1 : 0}
      className={className}
      title={title}
      onClick={() => !disabled && onPick()}
      onKeyDown={(e) => {
        if (!disabled && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          onPick();
        }
      }}
    >
      {children}
    </div>
  );
}

const Check = () => (
  <span className="menu-check" aria-hidden="true">
    ✓
  </span>
);

/**
 * A quiet chip ("Sonnet · Low", "Gemini 3.8 Flash Low") that opens a menu in two steps:
 * first the AI service, then that service's models, then the effort when the chosen
 * model takes one. A service that is not logged in shows a padlock and "Log in".
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
  const groups = groupByProvider(models);
  const current = models.find((m) => m.id === selected);
  const { open, setOpen, root } = usePopover();
  // The service whose models are listed: the chosen model's, until another is picked.
  const [viewed, setViewed] = useState(current?.provider ?? groups[0]?.[0] ?? "");

  const [name, level] = current ? chipName(current) : ["Default model", ""];
  const takesEffort = !!onEffortChange && !!current && !hasBuiltInLevel(current.id);
  const chipLevel = level || (takesEffort && effort ? effortLabel(effort) : "");
  const listed = groups.find(([p]) => p === viewed)?.[1] ?? groups[0]?.[1] ?? [];
  const showServices = groups.length > 1;

  return (
    <div className="model-menu" ref={root}>
      <button
        type="button"
        className="model-chip"
        aria-label={`Model: ${current?.label ?? name}${chipLevel && !level ? `, ${chipLevel} effort` : ""}`}
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => {
          // Opening shows the chosen model's service; the person can then browse others.
          if (!open) setViewed(current?.provider ?? groups[0]?.[0] ?? "");
          setOpen((o) => !o);
        }}
      >
        {name}
        {chipLevel && <span className="model-chip-effort">{chipLevel}</span>}
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
          <path d="M3 4.5 6 7.5 9 4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </button>
      {open && (
        <div className="menu menu-up" role="menu" aria-label="Model and effort">
          {showServices && (
            <div role="group" aria-label="AI service" className="menu-group">
              <div className="menu-head" aria-hidden="true">
                AI service
              </div>
              {groups.map(([provider, group]) => {
                // A started session stays with its provider: the others are not a login matter.
                const other = !!sessionProvider && provider !== sessionProvider;
                const locked = !other && group.every((m) => m.locked);
                return (
                  <div key={provider} className="menu-service-row">
                    <Item
                      checked={provider === viewed}
                      disabled={other || locked}
                      title={
                        other
                          ? `This session uses ${sessionProvider}; start a new session to use ${provider}`
                          : locked
                            ? `Log in to use ${provider}`
                            : undefined
                      }
                      onPick={() => setViewed(provider)}
                    >
                      <span>{provider}</span>
                      {other && <span className="menu-note">new session</span>}
                      {locked && (
                        <span className="menu-lock-icon" aria-hidden="true">
                          🔒
                        </span>
                      )}
                      {provider === viewed && <Check />}
                    </Item>
                    {locked && (
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
                );
              })}
            </div>
          )}
          <div role="group" aria-label={viewed ? `${viewed} models` : "Models"} className="menu-group">
            <div className="menu-head" aria-hidden="true">
              {viewed ? `${viewed} models` : "Models"}
            </div>
            {listed.map((m) => (
              <Item
                key={m.id}
                checked={m.id === selected}
                disabled={m.locked}
                onPick={() => {
                  onChange?.(m.id);
                  setOpen(false);
                }}
              >
                <span>{m.label}</span>
                {m.id === selected && <Check />}
              </Item>
            ))}
          </div>
          {takesEffort && (current.provider ?? "") === viewed && (
            <div role="group" aria-label="Effort" className="menu-group">
              <div className="menu-head" aria-hidden="true">
                Effort
              </div>
              <div className="menu-segments">
                {EFFORTS.map((e) => (
                  <Item
                    key={e || "default"}
                    className="menu-segment"
                    checked={e === effort}
                    onPick={() => onEffortChange?.(e)}
                  >
                    {effortLabel(e)}
                  </Item>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
