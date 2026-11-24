// The types sent between the app and this UI. Those that come from Rust are
// generated into ./bindings by `cargo test` (ts-rs): edit the Rust type, never the
// generated file. Only the UI's own types are written here.

export type { BrokenSource } from "./bindings/BrokenSource";
export type { Cause } from "./bindings/Cause";
export type { ChatEvent } from "./bindings/ChatEvent";
export type { ChatEventPayload } from "./bindings/ChatEventPayload";
export type { Check as EnvCheck } from "./bindings/Check";
export type { DatabaseForm } from "./bindings/DatabaseForm";
export type { DatabaseInfo } from "./bindings/DatabaseInfo";
export type { Edition } from "./bindings/Edition";
export type { FixKind } from "./bindings/FixKind";
export type { FoundSource } from "./bindings/FoundSource";
export type { GithubStatus } from "./bindings/GithubStatus";
export type { Model } from "./bindings/Model";
export type { NewDatabase } from "./bindings/NewDatabase";
export type { OdooVersions } from "./bindings/OdooVersions";
export type { ProjectInfo } from "./bindings/ProjectInfo";
export type { ProviderStatus } from "./bindings/ProviderStatus";
export type { PublishedVersions } from "./bindings/PublishedVersions";
export type { Release as AppRelease } from "./bindings/Release";
export type { Report as EnvReport } from "./bindings/Report";
export type { Requirement } from "./bindings/Requirement";
export type { SessionData } from "./bindings/SessionData";
export type { SessionMeta } from "./bindings/SessionMeta";
export type { Settings } from "./bindings/Settings";
export type { SetupStatus } from "./bindings/SetupStatus";
export type { SourceState } from "./bindings/SourceState";
export type { Task as SetupTask } from "./bindings/Task";
export type { TranscriptEntry } from "./bindings/TranscriptEntry";
export type { VersionInfo } from "./bindings/VersionInfo";

/** Where the client's Odoo runs: Odoo Online or Odoo.sh. */
export type Hosting = "online" | "sh";

/** A session is addressed by its project folder and id. */
export interface SessionRef {
  project: string;
  sessionId: string;
}

/** What the separate diagram window shows. */
export interface DiagramTarget extends SessionRef {
  title: string;
}
