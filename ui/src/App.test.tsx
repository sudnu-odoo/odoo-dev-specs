import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { api } from "./lib/api";
import type { ChatEvent, ChatEventPayload, ProjectInfo, SessionData, SetupStatus } from "./lib/types";

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
    runningSessions: vi.fn(),
    listDatabases: vi.fn(),
    setSessionDatabase: vi.fn(),
    setupStatus: vi.fn(),
    onSetupProgress: vi.fn(),
    providerStatus: vi.fn(),
    installTools: vi.fn(),
    downloadOdoo: vi.fn(),
    useFoundSources: vi.fn(),
    databaseForm: vi.fn(),
    checkUpdate: vi.fn(),
    odooVersions: vi.fn(),
    odooPublishedVersions: vi.fn(),
    setSessionHosting: vi.fn(),
    setSessionVersion: vi.fn(),
    createDatabase: vi.fn(),
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
const readyComputer: SetupStatus = {
  system: { requirements: [], ready: true },
  versions: ["19.0"],
  found: [],
  aiReady: true,
  tasks: {},
};
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
  mocked.runningSessions.mockResolvedValue([]);
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
  mocked.listDatabases.mockResolvedValue([
    {
      name: "specs_db_sales",
      label: "Sales demo",
      version: "19.0",
      modules: ["sale_management"],
      demoData: true,
      enterprise: false,
      auto: false,
      createdAt: 0,
      lastUsed: 0,
      sizeBytes: 0,
      running: false,
    },
  ]);
  mocked.setupStatus.mockResolvedValue(readyComputer);
  mocked.checkUpdate.mockResolvedValue(null);
  mocked.odooVersions.mockResolvedValue({ downloaded: [], downloadsFolder: "/home/me/odoo-versions" });
  mocked.odooPublishedVersions.mockResolvedValue({
    available: ["saas-19.2", "19.0"],
    latestSaas: "saas-19.2",
  });
  mocked.setSessionHosting.mockResolvedValue(undefined);
  mocked.setSessionVersion.mockResolvedValue(undefined);
  mocked.onSetupProgress.mockResolvedValue(() => {});
  mocked.providerStatus.mockResolvedValue([]);
  mocked.setSessionDatabase.mockImplementation(async (ref, database) => {
    projects = projects.map((p) => ({
      ...p,
      sessions: p.sessions.map((s) => (s.id === ref.sessionId ? { ...s, database } : s)),
    }));
  });
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

  it("opening a session tied to Antigravity switches to one of its models", async () => {
    mocked.listModels.mockResolvedValue([
      { id: "sonnet", label: "Sonnet", provider: "Claude" },
      { id: "gemini-3.8-flash-high", label: "Gemini Flash (High)", provider: "Antigravity" },
    ]);
    projects = [
      {
        name: "Acme",
        sessions: [{ ...meta("s1", "Delivery window"), provider: "antigravity", started: true }],
      },
    ];
    await openSession("Delivery window");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^Model/ })).toHaveTextContent("Gemini FlashHigh"),
    );
    await userEvent.type(message(), "x{Enter}");
    expect(mocked.sendMessage).toHaveBeenCalledWith(
      { project: "Acme", sessionId: "s1" },
      "x",
      "gemini-3.8-flash-high",
      null,
    );
  });

  it("after the first message, the session keeps that provider", async () => {
    mocked.listModels.mockResolvedValue([
      { id: "sonnet", label: "Sonnet", provider: "Claude" },
      { id: "gemini-3.8-flash-high", label: "Gemini Flash (High)", provider: "Antigravity" },
    ]);
    await openSession("New spec");
    await userEvent.type(message(), "x{Enter}");
    emitEvent("s2", { kind: "finished", is_error: false, message: null });
    await userEvent.click(screen.getByRole("button", { name: /^Model/ }));
    expect(screen.getByRole("menuitemradio", { name: /Antigravity/ })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
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

  it("keeps a running session live while you work in another one", async () => {
    await openSession("Delivery window");
    await screen.findByText("Earlier question");
    await userEvent.type(message(), "Add a field{Enter}");
    // The backend knows s1 is busy from now on.
    mocked.runningSessions.mockResolvedValue(["s1"]);

    // Switch away: the other session is free to chat.
    await userEvent.click(screen.getByRole("button", { name: "New spec" }));
    await waitFor(() => expect(screen.queryByText("Earlier question")).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Send" })).toBeInTheDocument();
    emitEvent("s1", { kind: "text_delta", text: "Working on it" });
    expect(screen.queryByText("Working on it")).not.toBeInTheDocument();

    // Back: the reply so far and the stop button are there, and stopping works.
    await userEvent.click(screen.getByRole("button", { name: "Delivery window" }));
    expect(await screen.findByText("Working on it")).toBeInTheDocument();
    expect(screen.getByText("Add a field")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(mocked.cancelTurn).toHaveBeenCalledWith("s1");
  });

  it("offers Stop for a session the backend is still running, e.g. after a reload", async () => {
    mocked.runningSessions.mockResolvedValue(["s1"]);
    await openSession("Delivery window");
    expect(await screen.findByRole("button", { name: "Stop" })).toBeInTheDocument();
    emitEvent("s1", { kind: "text_delta", text: "still answering" });
    expect(screen.getByText("still answering")).toBeInTheDocument();
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

  it("after skipping an unfinished setup, a banner offers to open it again", async () => {
    mocked.setupStatus.mockResolvedValue({ ...readyComputer, aiReady: false });
    render(<App />);
    await screen.findByRole("heading", { name: "Get ready to write specs" });
    await userEvent.click(screen.getByRole("button", { name: "Skip for now" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/setup is not finished/i);
    await userEvent.click(screen.getByRole("button", { name: "Open setup" }));
    expect(screen.getByRole("heading", { name: "Get ready to write specs" })).toBeInTheDocument();
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

  it("shows the Databases page from the sidebar, and a session again when one is picked", async () => {
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "Databases" }));
    expect(await screen.findByRole("heading", { name: "Databases" })).toBeInTheDocument();
    expect(screen.getByRole("listitem", { name: "Sales demo" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Delivery window" }));
    expect(screen.queryByRole("heading", { name: "Databases" })).not.toBeInTheDocument();
    expect(await screen.findByRole("textbox", { name: "Message" })).toBeInTheDocument();
  });

  it("each session shows and changes the database its screenshots use", async () => {
    await openSession("Delivery window");
    const chip = await screen.findByRole("button", { name: "Database: Automatic" });
    await userEvent.click(chip);
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Sales demo" }));
    expect(mocked.setSessionDatabase).toHaveBeenCalledWith(
      { project: "Acme", sessionId: "s1" },
      "specs_db_sales",
    );
    expect(await screen.findByRole("button", { name: "Database: Sales demo" })).toBeInTheDocument();
  });

  it("a fresh computer opens the setup, which installs the tools and uses the Odoo it finds", async () => {
    const fresh: SetupStatus = {
      ...readyComputer,
      system: {
        ready: false,
        requirements: [
          { key: "postgresql", label: "PostgreSQL", ok: false, problem: "PostgreSQL is not installed." },
        ],
      },
      versions: [],
      found: [{ version: "19.0", community: "/home/me/odev/worktrees/19.0/odoo", enterprise: null }],
    };
    mocked.setupStatus.mockResolvedValue(fresh);
    mocked.installTools.mockImplementation(async () => {
      mocked.setupStatus.mockResolvedValue({ ...fresh, system: { ready: true, requirements: [] } });
    });
    mocked.useFoundSources.mockResolvedValue(["19.0"]);
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Get ready to write specs" })).toBeInTheDocument();
    expect(screen.getByRole("listitem", { name: "Tools" })).toHaveTextContent("Missing: PostgreSQL.");
    await userEvent.click(screen.getByRole("button", { name: "Set up everything" }));
    expect(mocked.installTools).toHaveBeenCalledTimes(1);
    expect(mocked.useFoundSources).toHaveBeenCalledTimes(1);
  });

  it("a ready computer goes straight to the app", async () => {
    render(<App />);
    await screen.findByRole("button", { name: "Delivery window" });
    expect(screen.queryByRole("heading", { name: "Get ready to write specs" })).not.toBeInTheDocument();
  });

  it("a session can make a new database, which then becomes its database", async () => {
    mocked.databaseForm.mockResolvedValue({ suggestedLabel: "db-19-4", enterpriseAvailable: false });
    mocked.createDatabase.mockResolvedValue({
      name: "specs_db_19_4",
      label: "db-19-4",
      version: "19.0",
      modules: [],
      demoData: true,
      enterprise: false,
      auto: false,
      createdAt: 0,
      lastUsed: 0,
      sizeBytes: 0,
      running: false,
    });
    await openSession("Delivery window");
    await userEvent.click(await screen.findByRole("button", { name: "Database: Automatic" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "New database…" }));
    const dialog = await screen.findByRole("dialog", { name: "New database" });
    await within(dialog).findByDisplayValue("db-19-4");
    await userEvent.click(within(dialog).getByRole("button", { name: "Create" }));
    expect(mocked.createDatabase).toHaveBeenCalledWith(expect.objectContaining({ label: "db-19-4" }));
    expect(mocked.setSessionDatabase).toHaveBeenCalledWith(
      { project: "Acme", sessionId: "s1" },
      "specs_db_19_4",
    );
  });

  it("choosing Odoo Online sets the latest SaaS version for the session", async () => {
    await openSession("Delivery window");
    await userEvent.click(await screen.findByRole("button", { name: "Hosting: not chosen" }));
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Odoo Online" }));
    expect(mocked.setSessionHosting).toHaveBeenCalledWith({ project: "Acme", sessionId: "s1" }, "online");
    expect(mocked.setSessionVersion).toHaveBeenCalledWith({ project: "Acme", sessionId: "s1" }, "saas-19.2");
  });
});
