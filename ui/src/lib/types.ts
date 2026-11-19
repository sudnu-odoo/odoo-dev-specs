// Mirrors of the Rust types sent over IPC (crates/specs-core, src-tauri/src/commands.rs).

export interface SessionMeta {
  id: string;
  title: string;
  provider: string;
  created_at: number;
  updated_at: number;
  started: boolean;
  /** The local Odoo database its screenshots use; null = Auto (not chosen yet). */
  database?: string | null;
  /** Where the client's Odoo runs: "online" (Odoo Online) or "sh" (Odoo.sh); null = not chosen. */
  hosting?: Hosting | null;
  /** The client's Odoo version, e.g. "19.0" or "saas-19.2"; null = not chosen. */
  version?: string | null;
}

export type Hosting = "online" | "sh";

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
  | { kind: "usage"; input_tokens: number; output_tokens: number }
  /** The provider's own conversation id (Antigravity); the backend stores it. */
  | { kind: "conversation"; id: string }
  | {
      kind: "plan_usage";
      five_hour: { utilization: number; resets_at: number } | null;
      seven_day: { utilization: number; resets_at: number } | null;
    }
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
  provider: "claude";
  defaultVersion: string;
  demoData: boolean;
}

/** One environment requirement (mirrors odoo_runtime::doctor::Check). */
export interface EnvCheck {
  key: string;
  label: string;
  status: "ok" | "warning" | "missing";
  detail: string;
  hint: string | null;
  required: boolean;
}

/** Mirrors odoo_runtime::doctor::Report. */
export interface EnvReport {
  checks: EnvCheck[];
}

/** A selectable AI model (mirrors specs_core::models::Model). */
export interface Model {
  id: string;
  label: string;
  /** Provider the model belongs to; models are grouped by it in the picker. */
  provider?: string;
  /** The provider isn't logged in: shown with a padlock and not selectable. */
  locked?: boolean;
}

/** Whether an AI provider is installed and logged in (mirrors specs_core::providers). */
export interface ProviderStatus {
  provider: "claude" | "antigravity";
  name: string;
  installed: boolean;
  loggedIn: boolean;
}

/** A local Odoo database (mirrors odoo_runtime::runtime::DatabaseInfo). */
export interface DatabaseInfo {
  name: string;
  label: string;
  version: string;
  modules: string[];
  demoData: boolean;
  enterprise: boolean;
  /** Made by the assistant for screenshots rather than by a person. */
  auto: boolean;
  createdAt: number;
  lastUsed: number;
  sizeBytes: number;
  running: boolean;
}

/** The "New database" form (mirrors odoo_runtime::runtime::NewDatabase). */
export interface NewDatabase {
  label: string;
  version: string;
  modules: string[];
  demoData: boolean;
  /** Run with the enterprise source; only where the computer has it. */
  enterprise: boolean;
}

/** What the "New database" form starts with for a version. */
export interface DatabaseForm {
  /** The next free name, e.g. "db-19-3". */
  suggestedLabel: string;
  enterpriseAvailable: boolean;
}

/** One thing the computer must provide (mirrors odoo_runtime::system::Requirement). */
export interface Requirement {
  key: string;
  label: string;
  ok: boolean;
  /** What is wrong, in plain words. */
  problem: string;
}

export type SourceState = { state: "missing" } | { state: "ready"; linked: boolean };

/** A long setup step (mirrors src-tauri setup::Task). */
export interface SetupTask {
  state: "running" | "done" | "failed" | "noAccess";
  /** 0..100 while downloading; null while waiting without progress. */
  progress: number | null;
  message: string;
}

/** Everything the setup screen shows (mirrors src-tauri setup::SetupStatus). */
export interface SetupStatus {
  version: string;
  system: { requirements: Requirement[]; ready: boolean };
  community: SourceState;
  enterprise: SourceState;
  /** GitHub's tool is installed; the app's key reads Odoo Community / Enterprise. */
  github: { installed: boolean; connected: boolean; enterprise: boolean };
  aiReady: boolean;
  tasks: Record<string, SetupTask>;
}

export type Edition = "community" | "enterprise";

/** A newer version of the app (mirrors src-tauri update::Release). */
export interface AppRelease {
  version: string;
  /** What changed: one "- title" line per change. */
  notes: string;
  debUrl: string;
}

/** A version on this computer (mirrors odoo_runtime VersionInfo). */
export interface VersionInfo {
  version: string;
  community: SourceState;
  enterprise: SourceState;
  /** Unix time in ms of the last update; 0 when unknown. */
  updated: number;
  sizeBytes: number;
}

/** The versions Odoo publishes and the ones on this computer. */
export interface OdooVersions {
  /** Newest first; empty until GitHub is connected. */
  available: string[];
  latestSaas: string | null;
  downloaded: VersionInfo[];
}
