import { describe, expect, it } from "vitest";
import { chatReducer, emptyChat, type ChatState } from "./chatState";
import type { ChatEvent } from "../lib/types";

const event = (state: ChatState, e: ChatEvent) => chatReducer(state, { type: "event", event: e });
const running: ChatState = {
  items: [{ role: "user", text: "hi" }],
  running: true,
  usage: { inputTokens: 0, outputTokens: 0 },
};

describe("chatReducer", () => {
  it("loads a saved transcript", () => {
    const state = chatReducer(running, {
      type: "loaded",
      transcript: [
        { role: "user", text: "hi" },
        { role: "tool", name: "Read", is_error: true },
        { role: "assistant", text: "hello" },
        { role: "error", message: "Stopped." },
      ],
    });
    expect(state).toEqual({
      running: false,
      usage: { inputTokens: 0, outputTokens: 0 },
      items: [
        { role: "user", text: "hi" },
        { role: "tool", name: "Read", status: "error" },
        { role: "assistant", text: "hello" },
        { role: "error", message: "Stopped." },
      ],
    });
  });

  it("marks the chat as running when the user sends", () => {
    expect(chatReducer(emptyChat, { type: "sent", text: "hi" })).toEqual(running);
  });

  it("joins text deltas into one reply", () => {
    let s = event(running, { kind: "text_delta", text: "Hel" });
    s = event(s, { kind: "text_delta", text: "lo" });
    expect(s.items).toEqual([
      { role: "user", text: "hi" },
      { role: "assistant", text: "Hello" },
    ]);
  });

  it("tracks tools by id, also in parallel", () => {
    let s = event(running, { kind: "tool_started", id: "a", name: "Read", input: {} });
    s = event(s, { kind: "tool_started", id: "b", name: "Grep", input: {} });
    s = event(s, { kind: "tool_finished", id: "a", is_error: false });
    expect(s.items.slice(1)).toEqual([
      { role: "tool", id: "a", name: "Read", status: "ok" },
      { role: "tool", id: "b", name: "Grep", status: "running" },
    ]);
    s = event(s, { kind: "text_delta", text: "after" });
    expect(s.items.at(-1)).toEqual({ role: "assistant", text: "after" });
  });

  it("accumulates token usage across turns", () => {
    let s = event(running, { kind: "usage", input_tokens: 100, output_tokens: 20 });
    expect(s.usage).toEqual({ inputTokens: 100, outputTokens: 20 });
    s = event(s, { kind: "usage", input_tokens: 50, output_tokens: 10 });
    expect(s.usage).toEqual({ inputTokens: 150, outputTokens: 30 });
  });

  it("records plan limits and keeps them when a transcript loads", () => {
    const plan = {
      five_hour: { utilization: 0.21, resets_at: 1794916800 },
      seven_day: null,
    };
    const s = event(running, { kind: "plan_usage", ...plan });
    expect(s.plan).toEqual(plan);
    expect(chatReducer(s, { type: "loaded", transcript: [] }).plan).toEqual(plan);
  });

  it("keeps the usage total when a new transcript loads", () => {
    const withUsage = { ...running, usage: { inputTokens: 500, outputTokens: 80 } };
    const loaded = chatReducer(withUsage, { type: "loaded", transcript: [] });
    expect(loaded.usage).toEqual({ inputTokens: 0, outputTokens: 0 });
  });

  it("stops running on success", () => {
    const s = event(running, { kind: "finished", is_error: false, message: null });
    expect(s).toEqual({ ...running, running: false });
  });

  it("shows the error when a turn fails", () => {
    const s = event(running, { kind: "finished", is_error: true, message: "Not logged in" });
    expect(s.running).toBe(false);
    expect(s.items.at(-1)).toEqual({ role: "error", message: "Not logged in" });
  });

  it("shows an error when sending itself fails", () => {
    const s = chatReducer(running, { type: "sendFailed", message: "Claude Code is not installed." });
    expect(s).toEqual({
      running: false,
      usage: { inputTokens: 0, outputTokens: 0 },
      items: [...running.items, { role: "error", message: "Claude Code is not installed." }],
    });
  });

  it("never mutates the previous state", () => {
    const before = structuredClone(running);
    event(running, { kind: "text_delta", text: "x" });
    expect(running).toEqual(before);
  });
});
