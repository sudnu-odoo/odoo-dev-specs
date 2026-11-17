import { useEffect, useRef, useState } from "react";
import type { PlanUsage, PlanWindow, TokenUsage } from "../chat/chatState";

interface UsageMeterProps {
  plan?: PlanUsage;
  usage: TokenUsage;
}

const percent = (w: PlanWindow) => Math.round(w.utilization * 100);

/** "resets in 2h 14m", or a weekday and time when it is more than a day away. */
export function resetText(resetsAt: number, now = Date.now() / 1000): string {
  const secs = Math.max(0, resetsAt - now);
  if (secs >= 86400) {
    const d = new Date(resetsAt * 1000);
    return `resets ${d.toLocaleDateString([], { weekday: "short" })} ${d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  }
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return `resets in ${h > 0 ? `${h}h ` : ""}${m}m`;
}

/** Bar colour warms up as the limit gets close. */
const level = (p: number) => (p >= 90 ? "hot" : p >= 70 ? "warm" : "");

export function UsageBar({ label, window }: { label: string; window: PlanWindow }) {
  const p = percent(window);
  return (
    <div className="usage-bar">
      <div className="usage-bar-row">
        <span>{label}</span>
        <small>
          {p}% · {resetText(window.resets_at)}
        </small>
      </div>
      <div
        className={`usage-track ${level(p)}`}
        role="meter"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={p}
      >
        <i style={{ width: `${p}%` }} />
      </div>
    </div>
  );
}

/**
 * The small line under the composer: a mini bar with the session limit, which opens
 * a card with every limit window. Before the CLI reports limits, raw tokens are shown.
 */
export default function UsageMeter({ plan, usage }: UsageMeterProps) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

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

  const session = plan?.five_hour;
  if (!session) {
    if (usage.inputTokens + usage.outputTokens === 0) return null;
    return (
      <span className="usage-line" title="Tokens used in this session">
        {usage.inputTokens.toLocaleString("en-US")} in · {usage.outputTokens.toLocaleString("en-US")} out
        tokens
      </span>
    );
  }

  const p = percent(session);
  return (
    <div className="usage-meter" ref={root}>
      <button
        type="button"
        className="usage-line"
        aria-label={`Usage: ${p}% of session limit`}
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <span
          className={`usage-mini ${level(p)}`}
          role="meter"
          aria-label="Current session"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={p}
        >
          <i style={{ width: `${p}%` }} />
        </span>
        <span aria-hidden="true">{p}% of session limit</span>
      </button>
      {open && (
        <div className="usage-card" role="dialog" aria-label="Usage">
          <h3>Usage</h3>
          <UsageBar label="Current session" window={session} />
          {plan?.seven_day && <UsageBar label="Weekly · all models" window={plan.seven_day} />}
        </div>
      )}
    </div>
  );
}
