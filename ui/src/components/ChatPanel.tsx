import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent, type ReactNode } from "react";
import type { ChatItem, PlanUsage, TokenUsage } from "../chat/chatState";
import type { Model } from "../lib/types";
import Markdown from "./Markdown";
import ModelMenu from "./ModelMenu";
import ToolSteps from "./ToolSteps";
import UsageMeter from "./UsageMeter";

export interface ChatPanelProps {
  items: ChatItem[];
  running: boolean;
  hasSession: boolean;
  onSend: (text: string) => void;
  onStop: () => void;
  models?: Model[];
  selectedModel?: string;
  onModelChange?: (id: string) => void;
  /** Reasoning effort ("" = the CLI's default). */
  effort?: string;
  onEffortChange?: (effort: string) => void;
  /** Log in to a provider whose models are locked. */
  onLogin?: (provider: string) => void;
  /** Provider the open session is tied to (after its first message). */
  sessionProvider?: string;
  usage?: TokenUsage;
  /** Subscription limit usage; when known it replaces the raw token count. */
  plan?: PlanUsage;
  /** More chips for the composer bar, after the model (e.g. the session's database). */
  composerExtras?: ReactNode;
}

const NO_USAGE: TokenUsage = { inputTokens: 0, outputTokens: 0 };

const TOOL_STATUS_LABEL = { running: "…", ok: "✓", error: "✗" } as const;

export default function ChatPanel({
  items,
  running,
  hasSession,
  onSend,
  onStop,
  models = [],
  selectedModel = "",
  onModelChange,
  effort = "",
  onEffortChange,
  onLogin,
  sessionProvider,
  usage = NO_USAGE,
  plan,
  composerExtras,
}: ChatPanelProps) {
  const [draft, setDraft] = useState("");
  const bottom = useRef<HTMLDivElement>(null);
  // Until the first words of the answer arrive, show that the assistant is working.
  // (A running step already shows that it is working.)
  const lastItem = items.at(-1);
  const thinking =
    running &&
    lastItem?.role !== "assistant" &&
    !(lastItem?.role === "tool" && lastItem.status === "running");

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
        {groupSteps(items).map((group, i) =>
          Array.isArray(group) ? <ToolSteps key={i} steps={group} /> : <Message key={i} item={group} />,
        )}
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
          <div className="composer-chips">
            {models.length > 0 && (
              <ModelMenu
                models={models}
                selected={selectedModel}
                disabled={running || !hasSession}
                onChange={onModelChange}
                effort={effort}
                onEffortChange={onEffortChange}
                onLogin={onLogin}
                sessionProvider={sessionProvider}
              />
            )}
            {hasSession && composerExtras}
          </div>
          {running ? (
            <button
              type="button"
              className="send-btn send-btn-stop"
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
              className="send-btn"
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
      <div className="composer-footer">
        <UsageMeter plan={plan} usage={usage} />
      </div>
    </section>
  );
}

type ToolItem = Extract<ChatItem, { role: "tool" }>;

/** Consecutive tool steps become one group, shown as a single line. */
function groupSteps(items: ChatItem[]): (ChatItem | ToolItem[])[] {
  const out: (ChatItem | ToolItem[])[] = [];
  for (const item of items) {
    const last = out[out.length - 1];
    if (item.role === "tool" && Array.isArray(last)) last.push(item);
    else out.push(item.role === "tool" ? [item] : item);
  }
  return out;
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
