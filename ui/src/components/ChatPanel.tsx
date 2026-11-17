import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import type { ChatItem, PlanUsage, TokenUsage } from "../chat/chatState";
import type { Model } from "../lib/types";
import Markdown from "./Markdown";

export interface ChatPanelProps {
  items: ChatItem[];
  running: boolean;
  hasSession: boolean;
  onSend: (text: string) => void;
  onStop: () => void;
  models?: Model[];
  selectedModel?: string;
  onModelChange?: (id: string) => void;
  usage?: TokenUsage;
  /** Subscription limit usage; when known it replaces the raw token count. */
  plan?: PlanUsage;
}

const NO_USAGE: TokenUsage = { inputTokens: 0, outputTokens: 0 };

const TOOL_STATUS_LABEL = { running: "…", ok: "✓", error: "✗" } as const;

/** Group digits so large token counts stay readable (1234 -> "1,234"). */
function formatTokens(n: number): string {
  return n.toLocaleString("en-US");
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

function modelOption(m: Model) {
  return (
    <option key={m.id} value={m.id} disabled={m.locked}>
      {m.label}
    </option>
  );
}

export default function ChatPanel({
  items,
  running,
  hasSession,
  onSend,
  onStop,
  models = [],
  selectedModel = "",
  onModelChange,
  usage = NO_USAGE,
  plan,
}: ChatPanelProps) {
  const [draft, setDraft] = useState("");
  const bottom = useRef<HTMLDivElement>(null);
  // Until the first words of the answer arrive, show that the assistant is working.
  const thinking = running && items.at(-1)?.role !== "assistant";

  useEffect(() => {
    bottom.current?.scrollIntoView?.({ block: "end" });
  }, [items, thinking]);

  function send(e?: FormEvent) {
    e?.preventDefault();
    const text = draft.trim();
    if (!text || running || !hasSession) return;
    onSend(text);
    setDraft("");
  }

  function onKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  return (
    <section className="chat">
      <div className="chat-messages">
        {!hasSession && (
          <div className="chat-empty">
            <h2>Describe the feature to spec</h2>
            <p>Open or create a session on the left to start.</p>
          </div>
        )}
        {hasSession && items.length === 0 && (
          <div className="chat-empty">
            <h2>Describe the feature to spec</h2>
            <p>
              The assistant reads the Odoo source and helps you turn the request into a clear spec on the
              right.
            </p>
          </div>
        )}
        {items.map((item, i) => (
          <Message key={i} item={item} />
        ))}
        {thinking && (
          <div role="status" className="chat-thinking">
            <span className="spinner" aria-hidden="true" /> Thinking…
          </div>
        )}
        <div ref={bottom} />
      </div>
      <form className="composer" onSubmit={send}>
        <textarea
          aria-label="Message"
          title="Enter to send · Shift+Enter for a new line"
          placeholder={
            hasSession
              ? "Describe the feature, e.g. a delivery window on sales orders…"
              : "Open a session first"
          }
          rows={2}
          value={draft}
          disabled={!hasSession}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={onKeyDown}
        />
        <div className="composer-bar">
          {models.length > 0 && (
            <select
              className="model-picker"
              aria-label="Model"
              value={selectedModel}
              disabled={running || !hasSession}
              onChange={(e) => onModelChange?.(e.target.value)}
            >
              {groupByProvider(models).map(([provider, group]) =>
                provider ? (
                  <optgroup key={provider} label={group[0].locked ? `${provider} 🔒` : provider}>
                    {group.map(modelOption)}
                  </optgroup>
                ) : (
                  group.map(modelOption)
                ),
              )}
            </select>
          )}
          {plan?.five_hour ? (
            <PlanMeter window={plan.five_hour} />
          ) : (
            usage.inputTokens + usage.outputTokens > 0 && (
              <span className="usage" title="Tokens used in this session">
                {formatTokens(usage.inputTokens)} in · {formatTokens(usage.outputTokens)} out tokens
              </span>
            )
          )}
          <span className="composer-spacer" />
          {running ? (
            <button
              type="button"
              className="round-btn round-btn-busy"
              aria-label="Stop"
              aria-busy="true"
              title="Stop"
              onClick={onStop}
            >
              <span className="stop-square" aria-hidden="true" />
            </button>
          ) : (
            <button
              type="submit"
              className="round-btn"
              aria-label="Send"
              title="Send"
              disabled={!hasSession || !draft.trim()}
            >
              <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
                <path
                  d="M12 19V5M5 12l7-7 7 7"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2.4"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </button>
          )}
        </div>
      </form>
    </section>
  );
}

/** "5-hour limit 21%", with the reset time as a tooltip. */
function PlanMeter({ window }: { window: { utilization: number; resets_at: number } }) {
  const percent = Math.round(window.utilization * 100);
  const resets = new Date(window.resets_at * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
  return (
    <span
      role="meter"
      className="usage"
      aria-label="5-hour limit"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent}
      title={`Resets at ${resets}`}
    >
      5-hour limit {percent}%
    </span>
  );
}

function Message({ item }: { item: ChatItem }) {
  switch (item.role) {
    case "user":
      return <div className="msg msg-user">{item.text}</div>;
    case "assistant":
      return (
        <div className="msg msg-assistant markdown">
          <Markdown text={item.text} />
        </div>
      );
    case "tool":
      return (
        <div className="msg-tool" data-status={item.status}>
          {item.status === "running" ? <span className="spinner spinner-small" aria-hidden="true" /> : null}
          {item.status === "running" ? "" : TOOL_STATUS_LABEL[item.status]} {item.name}
        </div>
      );
    case "error":
      return (
        <div role="alert" className="msg msg-error">
          {item.message}
        </div>
      );
  }
}
