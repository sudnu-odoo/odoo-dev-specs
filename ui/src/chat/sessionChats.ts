import type { ChatEvent, TranscriptEntry } from "../lib/types";
import { chatReducer, emptyChat, type ChatAction, type ChatState, type PlanUsage } from "./chatState";

/**
 * One chat per session, so a session keeps answering (and stays stoppable) while the
 * user looks at another one. Plan usage is account-wide, so it is kept once.
 */
export interface SessionChats {
  chats: Record<string, ChatState>;
  plan?: PlanUsage;
}

export type SessionChatsAction =
  /** A session was opened: its saved transcript, and whether the backend is running it. */
  | { type: "opened"; sessionId: string; transcript: TranscriptEntry[]; running: boolean }
  /** Something the user did in a session (sent, sendFailed). */
  | { type: "chat"; sessionId: string; action: ChatAction }
  | { type: "event"; sessionId: string; event: ChatEvent };

export const emptySessionChats: SessionChats = { chats: {} };

export function sessionChatsReducer(state: SessionChats, action: SessionChatsAction): SessionChats {
  switch (action.type) {
    case "opened": {
      const live = state.chats[action.sessionId];
      // The transcript on disk only gets the reply when the turn ends: keep the live one.
      if (live?.running && action.running) return state;
      const loaded = chatReducer(emptyChat, { type: "loaded", transcript: action.transcript });
      return withChat(state, action.sessionId, { ...loaded, running: action.running });
    }
    case "chat": {
      const chat = state.chats[action.sessionId] ?? emptyChat;
      return withChat(state, action.sessionId, chatReducer(chat, action.action));
    }
    case "event": {
      const { event } = action;
      const plan =
        event.kind === "plan_usage" ? { five_hour: event.five_hour, seven_day: event.seven_day } : state.plan;
      const chat = state.chats[action.sessionId];
      // A session never opened here has no history to add to; it is loaded when opened.
      if (!chat) return plan === state.plan ? state : { ...state, plan };
      return { ...withChat(state, action.sessionId, chatReducer(chat, { type: "event", event })), plan };
    }
  }
}

function withChat(state: SessionChats, sessionId: string, chat: ChatState): SessionChats {
  return { ...state, chats: { ...state.chats, [sessionId]: chat } };
}
