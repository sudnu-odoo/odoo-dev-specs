// Typed wrappers around the Tauri commands in src-tauri/src/commands.rs.
// Components never call `invoke` directly.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ChatEventPayload,
  EnvReport,
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
  loadSession: (ref: SessionRef) => invoke<SessionData>("load_session", { ...ref }),
  loadScene: (ref: SessionRef) => invoke<string>("load_scene", { ...ref }),
  saveScene: (ref: SessionRef, scene: string) => invoke<void>("save_scene", { ...ref, scene }),
  sendMessage: (ref: SessionRef, text: string) => invoke<void>("send_message", { ...ref, text }),
  cancelTurn: (sessionId: string) => invoke<void>("cancel_turn", { sessionId }),
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
  /** In the diagram window: the main window selected another session. */
  onDiagramSession: (handler: (target: DiagramTarget) => void): Promise<UnlistenFn> =>
    listen<DiagramTarget>("diagram-session", (e) => handler(e.payload)),
  /** In the main window: the diagram window was closed. */
  onDiagramDocked: (handler: () => void): Promise<UnlistenFn> => listen("diagram-docked", () => handler()),
};
