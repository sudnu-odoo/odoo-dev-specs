import type { ChatEvent, TranscriptEntry } from "../lib/types";

export type ChatItem =
  | { role: "user"; text: string }
  | { role: "assistant"; text: string }
  | { role: "tool"; id?: string; name: string; status: "running" | "ok" | "error" }
  | { role: "error"; message: string };

export interface TokenUsage {
  inputTokens: number;
  outputTokens: number;
}

export interface ChatState {
  items: ChatItem[];
  /** True while the assistant is answering. */
  running: boolean;
  /** Tokens used so far in this session. */
  usage: TokenUsage;
}

export type ChatAction =
  | { type: "loaded"; transcript: TranscriptEntry[] }
  | { type: "sent"; text: string }
  | { type: "sendFailed"; message: string }
  | { type: "event"; event: ChatEvent };

const NO_USAGE: TokenUsage = { inputTokens: 0, outputTokens: 0 };
export const emptyChat: ChatState = { items: [], running: false, usage: NO_USAGE };

export function chatReducer(state: ChatState, action: ChatAction): ChatState {
  switch (action.type) {
    case "loaded":
      // Usage is live-only; the transcript on disk does not store it.
      return { running: false, items: action.transcript.map(fromTranscript), usage: NO_USAGE };
    case "sent":
      return { ...state, running: true, items: [...state.items, { role: "user", text: action.text }] };
    case "sendFailed":
      return {
        ...state,
        running: false,
        items: [...state.items, { role: "error", message: action.message }],
      };
    case "event":
      return applyEvent(state, action.event);
  }
}

function fromTranscript(entry: TranscriptEntry): ChatItem {
  if (entry.role === "tool") {
    return { role: "tool", name: entry.name, status: entry.is_error ? "error" : "ok" };
  }
  return entry;
}

function applyEvent(state: ChatState, event: ChatEvent): ChatState {
  const items = state.items;
  switch (event.kind) {
    case "text_delta": {
      const last = items.at(-1);
      if (last?.role === "assistant") {
        return { ...state, items: [...items.slice(0, -1), { ...last, text: last.text + event.text }] };
      }
      return { ...state, items: [...items, { role: "assistant", text: event.text }] };
    }
    case "tool_started":
      return {
        ...state,
        items: [...items, { role: "tool", id: event.id, name: event.name, status: "running" }],
      };
    case "tool_finished":
      return {
        ...state,
        items: items.map((item) =>
          item.role === "tool" && item.id === event.id
            ? { ...item, status: event.is_error ? "error" : "ok" }
            : item,
        ),
      };
    case "usage":
      return {
        ...state,
        usage: {
          inputTokens: state.usage.inputTokens + event.input_tokens,
          outputTokens: state.usage.outputTokens + event.output_tokens,
        },
      };
    case "finished":
      return {
        ...state,
        running: false,
        items: event.is_error
          ? [...items, { role: "error", message: event.message ?? "Something went wrong." }]
          : items,
      };
  }
}
