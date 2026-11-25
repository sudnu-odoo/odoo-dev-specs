// Typed wrappers around the Tauri commands in src-tauri/src/commands.rs.
// Components never call `invoke` directly.
import type { Attachment } from "./bindings/Attachment";
import type { GithubStatus } from "./bindings/GithubStatus";
import type { SearchHit } from "./bindings/SearchHit";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ChatEventPayload,
  AppRelease,
  DatabaseForm,
  Edition,
  SetupStatus,
  SetupTask,
  DatabaseInfo,
  Hosting,
  OdooVersions,
  PublishedVersions,
  NewDatabase,
  EnvReport,
  Model,
  ProviderStatus,
  Settings,
  DiagramTarget,
  ProjectInfo,
  SessionData,
  SessionMeta,
  SessionRef,
} from "./types";

export const api = {
  listProjects: () => invoke<ProjectInfo[]>("list_projects"),
  createProject: (name: string) => invoke<void>("create_project", { name }),
  createSession: (project: string, title: string) =>
    invoke<SessionMeta>("create_session", { project, title }),
  renameSession: (ref: SessionRef, title: string) => invoke<SessionMeta>("rename_session", { ...ref, title }),
  deleteSession: (ref: SessionRef) => invoke<void>("delete_session", { ...ref }),
  deleteProject: (project: string) => invoke<void>("delete_project", { project }),
  /** Put a session in a project ("" = none). */
  moveSession: (sessionId: string, project: string) => invoke<void>("move_session", { sessionId, project }),
  pinSession: (sessionId: string, pinned: boolean) => invoke<void>("pin_session", { sessionId, pinned }),
  pinProject: (project: string, pinned: boolean) => invoke<void>("pin_project", { project, pinned }),
  loadSession: (ref: SessionRef) => invoke<SessionData>("load_session", { ...ref }),
  loadScene: (ref: SessionRef) => invoke<string>("load_scene", { ...ref }),
  saveScene: (ref: SessionRef, scene: string) => invoke<void>("save_scene", { ...ref, scene }),
  sendMessage: (
    ref: SessionRef,
    text: string,
    model: string | null,
    effort: string | null,
    attachments?: Attachment[],
  ) => invoke<void>("send_message", { ...ref, text, model, effort, attachments: attachments ?? null }),
  /** Sessions matching a search, by title and by what was said in them (Ctrl+K). */
  searchSessions: (query: string) => invoke<SearchHit[]>("search_sessions", { query }),
  /** Prepare a PDF for the AI: its text (personal data removed) and a layout picture. */
  attachFile: (ref: SessionRef, path: string) => invoke<Attachment>("attach_file", { ...ref, path }),
  cancelTurn: (sessionId: string) => invoke<void>("cancel_turn", { sessionId }),
  /** Ids of the sessions whose assistant is answering right now. */
  runningSessions: () => invoke<string[]>("running_sessions"),
  onChatEvent: (handler: (payload: ChatEventPayload) => void): Promise<UnlistenFn> =>
    listen<ChatEventPayload>("chat-event", (e) => handler(e.payload)),
  /** Show the diagram in its own window (or point that window at another session). */
  openDiagramWindow: (ref: SessionRef, title: string) =>
    invoke<void>("open_diagram_window", { ...ref, title }),
  /** Close the diagram window; the main window shows the diagram again. */
  dockDiagramWindow: () => invoke<void>("dock_diagram_window"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  checkEnvironment: () => invoke<EnvReport>("check_environment"),
  listModels: () => invoke<Model[]>("list_models"),
  providerStatus: () => invoke<ProviderStatus[]>("provider_status"),
  /** Open the provider's login in a terminal window. */
  loginProvider: (provider: string) => invoke<void>("login_provider", { provider }),
  listDatabases: () => invoke<DatabaseInfo[]>("list_databases"),
  /** The suggested name and available editions for a new database of this version. */
  databaseForm: (version: string) => invoke<DatabaseForm>("database_form", { version }),
  createDatabase: (database: NewDatabase) => invoke<DatabaseInfo>("create_database", { database }),
  renameDatabase: (name: string, label: string) => invoke<void>("rename_database", { name, label }),
  duplicateDatabase: (name: string, label: string) =>
    invoke<DatabaseInfo>("duplicate_database", { name, label }),
  resetDatabase: (name: string) => invoke<void>("reset_database", { name }),
  deleteDatabase: (name: string) => invoke<void>("delete_database", { name }),
  /** Start the database's Odoo if needed; returns its login page address. */
  startDatabase: (name: string) => invoke<string>("start_database", { name }),
  stopDatabase: (name: string) => invoke<void>("stop_database", { name }),
  /** The database a session's screenshots use; null = Auto. */
  setSessionDatabase: (ref: SessionRef, database: string | null) =>
    invoke<void>("set_session_database", { ...ref, database }),
  /** A newer version on GitHub, or null (always null in development builds). */
  checkUpdate: () => invoke<AppRelease | null>("check_update"),
  /** Install it with the password window; the app restarts when done. */
  installUpdate: (release: AppRelease) => invoke<void>("install_update", { release }),
  setupStatus: () => invoke<SetupStatus>("setup_status"),
  /** Use the Odoo source found on the computer where it is; returns the versions. */
  useFoundSources: () => invoke<string[]>("use_found_sources"),
  /** Use a folder the person picked; returns the Odoo version found in it. */
  useSourceFolder: (path: string) => invoke<string>("use_source_folder", { path }),
  /** Whether the app can read Odoo's repositories (needed only to download versions). */
  githubStatus: () => invoke<GithubStatus>("github_status_cmd"),
  /** Reinstall PostgreSQL's programs (its databases are kept); progress as the "tools" task. */
  repairPostgres: () => invoke<void>("repair_postgres"),
  /** Stop using a version's folder (e.g. one that was moved); its files stay. */
  forgetSource: (version: string, edition: Edition) => invoke<void>("forget_source", { version, edition }),
  /** Install a version's Python packages in the background (task "python:<version>"). */
  prepareOdoo: (version: string) => invoke<void>("prepare_odoo", { version }),
  /** Every check as plain text for support, without personal paths. */
  setupReport: () => invoke<string>("setup_report"),
  /** Install and start what the computer is missing (one password prompt). */
  installTools: () => invoke<void>("install_tools"),
  /** Start a background download; progress arrives through onSetupProgress. */
  downloadOdoo: (version: string, edition: Edition) => invoke<void>("download_odoo", { version, edition }),
  /** Make the app's SSH key and add it to GitHub with one sign-in (a terminal window). */
  githubConnect: () => invoke<void>("github_connect"),
  odooVersions: () => invoke<OdooVersions>("odoo_versions"),
  odooPublishedVersions: () => invoke<PublishedVersions>("odoo_published_versions"),
  removeOdooVersion: (version: string) => invoke<void>("remove_odoo_version", { version }),
  updateOdooVersion: (version: string) => invoke<void>("update_odoo_version", { version }),
  setSessionVersion: (ref: SessionRef, version: string | null) =>
    invoke<void>("set_session_version", { ...ref, version }),
  useOdooFolder: (version: string, edition: Edition, path: string) =>
    invoke<void>("use_odoo_folder", { version, edition, path }),
  onSetupProgress: (handler: (update: { key: string; task: SetupTask }) => void): Promise<UnlistenFn> =>
    listen<{ key: string; task: SetupTask }>("setup-progress", (e) => handler(e.payload)),
  setSessionHosting: (ref: SessionRef, hosting: Hosting | null) =>
    invoke<void>("set_session_hosting", { ...ref, hosting }),
  /** In the diagram window: the main window selected another session. */
  onDiagramSession: (handler: (target: DiagramTarget) => void): Promise<UnlistenFn> =>
    listen<DiagramTarget>("diagram-session", (e) => handler(e.payload)),
  /** In the main window: the diagram window was closed. */
  onDiagramDocked: (handler: () => void): Promise<UnlistenFn> => listen("diagram-docked", () => handler()),
};
