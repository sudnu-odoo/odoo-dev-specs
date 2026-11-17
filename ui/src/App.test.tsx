import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { api } from "./lib/api";
import type { ChatEvent, ChatEventPayload, ProjectInfo, SessionData } from "./lib/types";

vi.mock("./lib/api", () => ({
  api: {
    listProjects: vi.fn(),
    createProject: vi.fn(),
    createSession: vi.fn(),
    renameSession: vi.fn(),
    deleteSession: vi.fn(),
    loadSession: vi.fn(),
    saveScene: vi.fn(),
    sendMessage: vi.fn(),
    cancelTurn: vi.fn(),
    onChatEvent: vi.fn(),
    loadScene: vi.fn(),
    openDiagramWindow: vi.fn(),
    dockDiagramWindow: vi.fn(),
    onDiagramDocked: vi.fn(),
    getSettings: vi.fn(),
    checkEnvironment: vi.fn(),
    saveSettings: vi.fn(),
    listModels: vi.fn(),
  },
}));
// Settings panel has its own test; here we only check it opens.
vi.mock("./components/SettingsPanel", () => ({
  default: ({ onClose }: { onClose: () => void }) => (
    <div data-testid="settings-panel">
      <button onClick={onClose}>close-settings</button>
    </div>
  ),
}));
vi.mock("./lib/confirm", () => ({ confirmAction: vi.fn().mockResolvedValue(true) }));
// jsdom has no layout: every element measures 0x0, so the panel library thinks each
// click lands on its resize handle and swallows it. Plain containers instead.
vi.mock("react-resizable-panels", () => ({
  PanelGroup: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Panel: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  PanelResizeHandle: () => null,
}));
// The real canvas needs a browser; its own behaviour is covered elsewhere.
vi.mock("./components/ExcalidrawPanel", () => ({
  default: ({ name, actions }: { name: string; actions?: React.ReactNode }) => (
    <div>
      <div data-testid="diagram">{name}</div>
      {actions}
    </div>
  ),
}));

const mocked = vi.mocked(api);
const meta = (id: string, title: string) => ({
  id,
  title,
  provider: "claude",
  created_at: 0,
  updated_at: 0,
  started: false,
});
let projects: ProjectInfo[];
// Like the real event bus: every subscriber (the app and the diagram) gets each event.
let chatListeners: ((payload: ChatEventPayload) => void)[] = [];
const emit = (payload: ChatEventPayload) => chatListeners.forEach((listener) => listener(payload));
let docked: () => void;
const emitEvent = (sessionId: string, event: ChatEvent) => act(() => emit({ sessionId, event }));

beforeEach(() => {
  vi.clearAllMocks();
  projects = [{ name: "Acme", sessions: [meta("s1", "Delivery window"), meta("s2", "New spec")] }];
  mocked.listProjects.mockImplementation(async () => projects);
  mocked.loadSession.mockImplementation(
    async (ref): Promise<SessionData> => ({
      meta: projects[0].sessions.find((s) => s.id === ref.sessionId)!,
      scene: '{"type":"excalidraw","elements":[]}',
      transcript: ref.sessionId === "s1" ? [{ role: "user", text: "Earlier question" }] : [],
    }),
  );
  mocked.sendMessage.mockResolvedValue(undefined);
  mocked.renameSession.mockImplementation(async (_ref, title) => meta("s2", title));
  chatListeners = [];
  mocked.onChatEvent.mockImplementation(async (handler) => {
    chatListeners.push(handler);
    return () => {
      chatListeners = chatListeners.filter((l) => l !== handler);
    };
  });
  mocked.onDiagramDocked.mockImplementation(async (handler) => {
    docked = handler;
    return () => {};
  });
  mocked.loadScene.mockResolvedValue('{"type":"excalidraw","elements":[]}');
  mocked.checkEnvironment.mockResolvedValue({ checks: [] });
  mocked.listModels.mockResolvedValue([
    { id: "sonnet", label: "Sonnet (balanced)" },
    { id: "opus", label: "Opus (most capable)" },
  ]);
  mocked.getSettings.mockResolvedValue({ provider: "claude", defaultVersion: "19.0", demoData: true });
  mocked.openDiagramWindow.mockResolvedValue(undefined);
  mocked.dockDiagramWindow.mockResolvedValue(undefined);
});

async function openSession(title: string) {
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: title }));
}

const message = () => screen.getByRole("textbox", { name: "Message" });

describe("App", () => {
  it("opens a session with its history and diagram", async () => {
    await openSession("Delivery window");
    expect(await screen.findByText("Earlier question")).toBeInTheDocument();
    expect(screen.getByTestId("diagram")).toHaveTextContent("Delivery window");
    expect(mocked.loadSession).toHaveBeenCalledWith({ project: "Acme", sessionId: "s1" });
  });

  it("never defaults to a locked model", async () => {
    mocked.listModels.mockResolvedValue([
      { id: "gem", label: "Gemini", provider: "Antigravity", locked: true },
      { id: "sonnet", label: "Sonnet", provider: "Claude" },
    ]);
    await openSession("Delivery window");
    await screen.findByText("Earlier question");
    expect(screen.getByRole("button", { name: /^Model/ })).toHaveTextContent("Sonnet");
  });

  it("sends a message and streams the reply", async () => {
    await openSession("Delivery window");
    await screen.findByText("Earlier question");
    await userEvent.type(message(), "Add a field{Enter}");

    expect(mocked.sendMessage).toHaveBeenCalledWith(
      { project: "Acme", sessionId: "s1" },
      "Add a field",
      "sonnet",
      null,
    );
    expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();

    emitEvent("s1", { kind: "text_delta", text: "Hel" });
    emitEvent("s1", { kind: "text_delta", text: "lo" });
    expect(screen.getByText("Hello")).toBeInTheDocument();

    emitEvent("s1", { kind: "finished", is_error: false, message: null });
    expect(await screen.findByRole("button", { name: "Send" })).toBeInTheDocument();
    await waitFor(() => expect(mocked.listProjects).toHaveBeenCalledTimes(2));
  });

  it("stops a running answer", async () => {
    await openSession("Delivery window");
    await userEvent.type(message(), "x{Enter}");
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(mocked.cancelTurn).toHaveBeenCalledWith("s1");
  });

  it("ignores events from other sessions", async () => {
    await openSession("Delivery window");
    await userEvent.type(message(), "x{Enter}");
    emitEvent("other", { kind: "text_delta", text: "Not for you" });
    expect(screen.queryByText("Not for you")).not.toBeInTheDocument();
  });

  it("names a new session after its first message", async () => {
    await openSession("New spec");
    await userEvent.type(message(), "Delivery slip changes\nmore details{Enter}");
    expect(mocked.renameSession).toHaveBeenCalledWith(
      { project: "Acme", sessionId: "s2" },
      "Delivery slip changes",
    );
  });

  it("shows why a message could not be sent", async () => {
    mocked.sendMessage.mockRejectedValue("Claude Code is not installed.");
    await openSession("Delivery window");
    await userEvent.type(message(), "x{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("Claude Code is not installed.");
    expect(screen.getByRole("button", { name: "Send" })).toBeInTheDocument();
  });

  it("creates a session and opens it", async () => {
    mocked.createSession.mockImplementation(async () => {
      projects = [{ name: "Acme", sessions: [meta("s3", "New spec"), ...projects[0].sessions] }];
      return meta("s3", "New spec");
    });
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "New session in Acme" }));
    expect(mocked.createSession).toHaveBeenCalledWith("Acme", "New spec");
    await waitFor(() =>
      expect(mocked.loadSession).toHaveBeenCalledWith({ project: "Acme", sessionId: "s3" }),
    );
  });

  it("renames the open session and its diagram", async () => {
    mocked.renameSession.mockImplementation(async (_ref, title) => {
      projects = [{ name: "Acme", sessions: [meta("s1", title), meta("s2", "New spec")] }];
      return meta("s1", title);
    });
    await openSession("Delivery window");
    await screen.findByText("Earlier question");
    await userEvent.click(screen.getByRole("button", { name: "Rename Delivery window" }));
    const field = screen.getByRole("textbox", { name: "Session name" });
    await userEvent.clear(field);
    await userEvent.type(field, "Delivery slots{Enter}");
    expect(mocked.renameSession).toHaveBeenCalledWith({ project: "Acme", sessionId: "s1" }, "Delivery slots");
    expect(await screen.findByTestId("diagram")).toHaveTextContent("Delivery slots");
    expect(await screen.findByRole("button", { name: "Delivery slots" })).toBeInTheDocument();
  });

  it("closes the open session when it is deleted", async () => {
    mocked.deleteSession.mockImplementation(async () => {
      projects = [{ name: "Acme", sessions: [meta("s2", "New spec")] }];
    });
    await openSession("Delivery window");
    await screen.findByText("Earlier question");
    await userEvent.click(screen.getByRole("button", { name: "Delete Delivery window" }));
    await waitFor(() => expect(screen.queryByText("Earlier question")).not.toBeInTheDocument());
    expect(mocked.deleteSession).toHaveBeenCalledWith({ project: "Acme", sessionId: "s1" });
    expect(message()).toBeDisabled();
  });

  it("moves the diagram to its own window and back", async () => {
    await openSession("Delivery window");
    await userEvent.click(await screen.findByRole("button", { name: "Open in new window" }));
    expect(mocked.openDiagramWindow).toHaveBeenCalledWith(
      { project: "Acme", sessionId: "s1" },
      "Delivery window",
    );
    expect(screen.queryByTestId("diagram")).not.toBeInTheDocument();
    expect(message()).toBeEnabled();

    await userEvent.click(screen.getByRole("button", { name: "Bring diagram back" }));
    expect(mocked.dockDiagramWindow).toHaveBeenCalled();
    act(() => docked());
    expect(await screen.findByTestId("diagram")).toHaveTextContent("Delivery window");
  });

  it("shows the diagram again when its window is closed directly", async () => {
    await openSession("Delivery window");
    await userEvent.click(await screen.findByRole("button", { name: "Open in new window" }));
    act(() => docked());
    expect(await screen.findByTestId("diagram")).toBeInTheDocument();
  });

  it("keeps the separate diagram window on the selected session", async () => {
    await openSession("Delivery window");
    await userEvent.click(await screen.findByRole("button", { name: "Open in new window" }));
    await userEvent.click(screen.getByRole("button", { name: "New spec" }));
    await waitFor(() =>
      expect(mocked.openDiagramWindow).toHaveBeenLastCalledWith(
        { project: "Acme", sessionId: "s2" },
        "New spec",
      ),
    );
  });
  it("opens and closes the settings panel", async () => {
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(await screen.findByTestId("settings-panel")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "close-settings" }));
    await waitFor(() => expect(screen.queryByTestId("settings-panel")).not.toBeInTheDocument());
  });

  it("warns on first run when a required tool is missing", async () => {
    mocked.checkEnvironment.mockResolvedValue({
      checks: [
        {
          key: "git",
          label: "Git",
          status: "missing",
          detail: "not found",
          hint: "Install git.",
          required: true,
        },
      ],
    });
    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent(/setup is incomplete/i);
    // Opening settings from the warning shows the details.
    await userEvent.click(screen.getByRole("button", { name: /open settings/i }));
    expect(await screen.findByTestId("settings-panel")).toBeInTheDocument();
  });

  it("shows no setup warning when everything is ready", async () => {
    mocked.checkEnvironment.mockResolvedValue({
      checks: [
        { key: "git", label: "Git", status: "ok", detail: "/usr/bin/git", hint: null, required: true },
      ],
    });
    render(<App />);
    await screen.findByRole("button", { name: "Settings" });
    expect(screen.queryByText(/setup is incomplete/i)).not.toBeInTheDocument();
  });
});
