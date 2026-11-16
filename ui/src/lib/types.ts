// Mirrors of the Rust types sent over IPC (crates/specs-core, src-tauri/src/commands.rs).

export interface SessionMeta {
  id: string;
  title: string;
  provider: string;
  created_at: number;
  updated_at: number;
  started: boolean;
}

export interface ProjectInfo {
  name: string;
  sessions: SessionMeta[];
}

export type TranscriptEntry =
  | { role: "user"; text: string }
  | { role: "assistant"; text: string }
  | { role: "tool"; name: string; is_error: boolean }
  | { role: "error"; message: string };

export interface SessionData {
  meta: SessionMeta;
  /** The .excalidraw JSON as stored on disk. */
  scene: string;
  transcript: TranscriptEntry[];
}

export type ChatEvent =
  | { kind: "text_delta"; text: string }
  | { kind: "tool_started"; id: string; name: string; input: unknown }
  | { kind: "tool_finished"; id: string; is_error: boolean }
  | { kind: "finished"; is_error: boolean; message: string | null };

export interface ChatEventPayload {
  sessionId: string;
  event: ChatEvent;
}

/** A session is addressed by its project folder and id. */
export interface SessionRef {
  project: string;
  sessionId: string;
}

/** What the separate diagram window shows. */
export interface DiagramTarget extends SessionRef {
  title: string;
}

/** Mirrors specs_core::settings::Settings. */
export interface Settings {
  provider: "claude" | "gemini";
  defaultVersion: string;
  demoData: boolean;
}

/** One environment requirement (mirrors odev_core::doctor::Check). */
export interface EnvCheck {
  key: string;
  label: string;
  status: "ok" | "warning" | "missing";
  detail: string;
  hint: string | null;
  required: boolean;
}

/** Mirrors odev_core::doctor::Report. */
export interface EnvReport {
  checks: EnvCheck[];
}
