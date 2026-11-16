import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import type { ChatItem } from "../chat/chatState";
import Markdown from "./Markdown";

export interface ChatPanelProps {
  items: ChatItem[];
  running: boolean;
  hasSession: boolean;
  onSend: (text: string) => void;
  onStop: () => void;
}

const TOOL_STATUS_LABEL = { running: "…", ok: "✓", error: "✗" } as const;

export default function ChatPanel({ items, running, hasSession, onSend, onStop }: ChatPanelProps) {
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
          <span className="composer-hint">Enter to send · Shift+Enter for a new line</span>
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
