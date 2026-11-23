import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent, type ReactNode } from "react";
import type { ChatItem, PlanUsage, TokenUsage } from "../chat/chatState";
import type { Attachment } from "../lib/bindings/Attachment";
import type { Model } from "../lib/types";
import Markdown from "./Markdown";
import ModelMenu from "./ModelMenu";
import ToolSteps from "./ToolSteps";
import UsageMeter from "./UsageMeter";

export interface ChatPanelProps {
  items: ChatItem[];
  running: boolean;
  hasSession: boolean;
  /** `attachments` only when some are attached to this message. */
  onSend: (text: string, attachments?: Attachment[]) => void;
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
  /** Claude's plan limits apply: false for Antigravity, which reports none. */
  showUsage?: boolean;
  /** More chips for the composer bar, after the model (e.g. the session's database). */
  composerExtras?: ReactNode;
  /** Pick and prepare a file to attach; null when the person cancels. */
  onAttach?: () => Promise<Attachment | null>;
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
  showUsage = true,
  onAttach,
}: ChatPanelProps) {
  const [draft, setDraft] = useState("");
  const [attached, setAttached] = useState<Attachment[]>([]);
  const [attaching, setAttaching] = useState(false);
  const [attachError, setAttachError] = useState<string | null>(null);
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
    if (attached.length) onSend(text, attached);
    else onSend(text);
    setDraft("");
    setAttached([]);
  }

  async function attach() {
    if (!onAttach) return;
    setAttaching(true);
    setAttachError(null);
    try {
      const file = await onAttach();
      if (file) setAttached((list) => [...list.filter((a) => a.textFile !== file.textFile), file]);
    } catch (e) {
      setAttachError(String(e));
    } finally {
      setAttaching(false);
    }
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
        {attached.length > 0 && (
          <div className="attachments" role="group" aria-label="Attached">
            {attached.map((a) => (
              <span key={a.textFile} className="attachment-chip">
                <span className="attachment-name">{a.name}</span>
                <small>{attachmentText(a)}</small>
                <button
                  type="button"
                  className="icon-btn"
                  aria-label={`Remove ${a.name}`}
                  onClick={() => setAttached((list) => list.filter((x) => x !== a))}
                >
                  ×
                </button>
              </span>
            ))}
          </div>
        )}
        {attachError && (
          <div role="alert" className="attachment-error">
            {attachError}
          </div>
        )}
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
            {onAttach && hasSession && (
              <button
                type="button"
                className="icon-btn attach-btn"
                aria-label="Attach a report (PDF)"
                title="Attach a report (PDF): a blank or sample one, never real client data"
                disabled={running || attaching}
                aria-busy={attaching || undefined}
                onClick={attach}
              >
                {attaching ? (
                  <span className="spinner spinner-small" aria-hidden="true" />
                ) : (
                  <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
                    <path
                      d="M10.5 4.5 5.4 9.6a1.4 1.4 0 0 0 2 2l5.3-5.3a2.8 2.8 0 0 0-4-4L3.4 7.6a4.2 4.2 0 0 0 6 6l4.1-4.1"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.3"
                      strokeLinecap="round"
                    />
                  </svg>
                )}
              </button>
            )}
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
      <div className="composer-footer">{showUsage && <UsageMeter plan={plan} usage={usage} />}</div>
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

/** "2 pages · 5 items removed", or that a scan only gives its layout. */
function attachmentText(a: Attachment): string {
  const pages = `${a.pages} page${a.pages === 1 ? "" : "s"}`;
  if (a.scanned) return `${pages} · scan: layout only`;
  return a.removed ? `${pages} · ${a.removed} items removed` : pages;
}
