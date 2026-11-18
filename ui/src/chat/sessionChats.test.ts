import { describe, expect, it } from "vitest";
import { emptySessionChats, sessionChatsReducer as reduce, type SessionChats } from "./sessionChats";

const opened = (s: SessionChats, sessionId: string, running = false) =>
  reduce(s, { type: "opened", sessionId, transcript: [{ role: "user", text: `hi ${sessionId}` }], running });

describe("sessionChatsReducer", () => {
  it("routes each event to its own session, open or not", () => {
    let s = opened(opened(emptySessionChats, "a"), "b");
    s = reduce(s, { type: "chat", sessionId: "a", action: { type: "sent", text: "go" } });
    s = reduce(s, { type: "event", sessionId: "a", event: { kind: "text_delta", text: "working" } });
    expect(s.chats.a.items.at(-1)).toEqual({ role: "assistant", text: "working" });
    expect(s.chats.a.running).toBe(true);
    expect(s.chats.b.items).toEqual([{ role: "user", text: "hi b" }]);
  });

  it("keeps the live chat of a running session when it is opened again", () => {
    let s = opened(emptySessionChats, "a");
    s = reduce(s, { type: "chat", sessionId: "a", action: { type: "sent", text: "go" } });
    s = reduce(s, { type: "event", sessionId: "a", event: { kind: "text_delta", text: "so far" } });
    s = opened(s, "a", true);
    expect(s.chats.a.items.at(-1)).toEqual({ role: "assistant", text: "so far" });
    expect(s.chats.a.running).toBe(true);
  });

  it("reloads a finished session from its saved transcript", () => {
    let s = opened(emptySessionChats, "a");
    s = reduce(s, { type: "chat", sessionId: "a", action: { type: "sent", text: "go" } });
    s = reduce(s, {
      type: "event",
      sessionId: "a",
      event: { kind: "finished", is_error: false, message: null },
    });
    s = opened(s, "a");
    expect(s.chats.a.items).toEqual([{ role: "user", text: "hi a" }]);
  });

  it("trusts the backend: a session it runs shows as running even without live state", () => {
    const s = opened(emptySessionChats, "a", true);
    expect(s.chats.a.running).toBe(true);
  });

  it("keeps plan usage account-wide, from whichever session reports it", () => {
    const plan = { five_hour: { utilization: 0.4, resets_at: 1 }, seven_day: null };
    const s = reduce(emptySessionChats, {
      type: "event",
      sessionId: "unknown",
      event: { kind: "plan_usage", ...plan },
    });
    expect(s.plan).toEqual(plan);
  });

  it("ignores events of sessions it never opened (no partial history)", () => {
    const s = reduce(emptySessionChats, {
      type: "event",
      sessionId: "x",
      event: { kind: "text_delta", text: "t" },
    });
    expect(s.chats.x).toBeUndefined();
  });
});
