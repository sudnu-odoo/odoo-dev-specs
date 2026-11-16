import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import type { ChatItem } from "../chat/chatState";

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

  useEffect(() => {
    bottom.current?.scrollIntoView?.({ block: "end" });
  }, [items]);

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
        <div ref={bottom} />
      </div>
      <form className="chat-input" onSubmit={send}>
        <textarea
          aria-label="Message"
          placeholder="e.g. Add a delivery window on sales orders, shown on the delivery slip…"
          rows={3}
          value={draft}
          disabled={!hasSession}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={onKeyDown}
        />
        {running ? (
          <button type="button" className="btn" onClick={onStop}>
            Stop
          </button>
        ) : (
          <button type="submit" className="btn btn-primary" disabled={!hasSession}>
            Send
          </button>
        )}
      </form>
    </section>
  );
}

function Message({ item }: { item: ChatItem }) {
  switch (item.role) {
    case "user":
      return <div className="msg msg-user">{item.text}</div>;
    case "assistant":
      return <div className="msg msg-assistant">{item.text}</div>;
    case "tool":
      return (
        <div className="msg-tool" data-status={item.status}>
          {TOOL_STATUS_LABEL[item.status]} {item.name}
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
