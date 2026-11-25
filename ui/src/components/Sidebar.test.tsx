import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import Sidebar, { type SidebarProps } from "./Sidebar";
import type { ProjectInfo } from "../lib/types";

const session = (id: string, title: string) => ({
  id,
  title,
  provider: "claude",
  created_at: 0,
  conversation_id: null,
  database: null,
  hosting: null,
  version: null,
  updated_at: 0,
  started: false,
});

const projects: ProjectInfo[] = [
  {
    name: "Acme",
    sessions: [
      { ...session("s1", "Delivery window"), hosting: "online", version: "saas-19.2" },
      session("s2", "Invoice layout"),
    ],
  },
  { name: "Globex", sessions: [] },
];
const withLoose: ProjectInfo[] = [{ name: "", sessions: [session("s3", "Quick question")] }, ...projects];

function setup(overrides: Partial<SidebarProps> = {}) {
  const props: SidebarProps = {
    projects,
    selected: { project: "Acme", sessionId: "s1" },
    onSelect: vi.fn(),
    onNewSession: vi.fn(),
    onSearch: vi.fn(),
    onCreateSession: vi.fn(),
    onDeleteSession: vi.fn(),
    onDeleteProject: vi.fn(),
    onNewProject: vi.fn(),
    onMoveSession: vi.fn(),
    onPinSession: vi.fn(),
    onPinProject: vi.fn(),
    onRenameSession: vi.fn().mockResolvedValue(undefined),
    onOpenSettings: vi.fn(),
    ...overrides,
  };
  render(<Sidebar {...props} />);
  return props;
}

describe("Sidebar", () => {
  it("shows each session's client hosting and version, and a spinner while it works", () => {
    setup({ running: ["s2"] });
    expect(screen.getByRole("button", { name: "Delivery window" })).toHaveAttribute(
      "title",
      expect.stringContaining("Online · 19.2"),
    );
    const invoice = screen.getByRole("button", { name: "Invoice layout" }).parentElement!;
    expect(within(invoice).getByRole("status", { name: "Working" })).toBeInTheDocument();
  });

  it("shows or hides a project's sessions when its name is clicked", async () => {
    setup();
    const acme = screen.getByRole("button", { name: "Acme" });
    expect(acme).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(acme);
    expect(acme).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("button", { name: "Delivery window" })).not.toBeInTheDocument();
    // Still possible to start a session in a closed project.
    expect(screen.getByRole("button", { name: "New session in Acme" })).toBeInTheDocument();
    await userEvent.click(acme);
    expect(screen.getByRole("button", { name: "Delivery window" })).toBeInTheDocument();
  });

  it("lists projects with their sessions and marks the open one", () => {
    setup();
    expect(screen.getByText("Acme")).toBeInTheDocument();
    expect(screen.getByText("Globex")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delivery window" })).toHaveAttribute("aria-current", "true");
    expect(screen.getByRole("button", { name: "Invoice layout" })).not.toHaveAttribute("aria-current");
  });

  it("opens a session on click", async () => {
    const props = setup();
    await userEvent.click(screen.getByRole("button", { name: "Invoice layout" }));
    expect(props.onSelect).toHaveBeenCalledWith({ project: "Acme", sessionId: "s2" });
  });

  it("asks for a new session and for the search, with their shortcuts shown", async () => {
    const props = setup();
    await userEvent.click(screen.getByRole("button", { name: "New session Ctrl N" }));
    expect(props.onNewSession).toHaveBeenCalled();
    const search = screen.getByRole("button", { name: "Search Ctrl K" });
    expect(search).toHaveTextContent("Ctrl K");
    await userEvent.click(search);
    expect(props.onSearch).toHaveBeenCalled();
  });

  it("starts a new session in a project", async () => {
    const props = setup();
    await userEvent.click(screen.getByRole("button", { name: "New session in Globex" }));
    expect(props.onCreateSession).toHaveBeenCalledWith("Globex");
  });

  it("renames a session from its rename button", async () => {
    const props = setup();
    await userEvent.click(screen.getByRole("button", { name: "Rename Invoice layout" }));
    const field = screen.getByRole("textbox", { name: "Session name" });
    expect(field).toHaveValue("Invoice layout");
    await userEvent.clear(field);
    await userEvent.type(field, "Invoice v2{Enter}");
    expect(props.onRenameSession).toHaveBeenCalledWith({ project: "Acme", sessionId: "s2" }, "Invoice v2");
    expect(props.onRenameSession).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("textbox", { name: "Session name" })).not.toBeInTheDocument();
  });

  it("renames on double-click and cancels with Escape", async () => {
    const props = setup();
    await userEvent.dblClick(screen.getByRole("button", { name: "Delivery window" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Session name" }), " X{Escape}");
    expect(props.onRenameSession).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Delivery window" })).toBeInTheDocument();
  });

  it("does not rename to an unchanged or blank title", async () => {
    const props = setup();
    await userEvent.click(screen.getByRole("button", { name: "Rename Invoice layout" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Session name" }), "{Enter}");
    await userEvent.click(screen.getByRole("button", { name: "Rename Delivery window" }));
    await userEvent.clear(screen.getByRole("textbox", { name: "Session name" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Session name" }), "   {Enter}");
    expect(props.onRenameSession).not.toHaveBeenCalled();
  });

  it("deletes a session", async () => {
    const props = setup();
    await userEvent.click(screen.getByRole("button", { name: "Delete Invoice layout" }));
    expect(props.onDeleteSession).toHaveBeenCalledWith({ project: "Acme", sessionId: "s2" });
    expect(props.onSelect).not.toHaveBeenCalled();
  });

  it("deletes a project from its header", async () => {
    const props = setup();
    await userEvent.click(screen.getByRole("button", { name: "Delete project Globex" }));
    expect(props.onDeleteProject).toHaveBeenCalledWith("Globex");
  });

  it("guides the user when there are no sessions yet", () => {
    setup({ projects: [], selected: null });
    expect(screen.getByText(/Start with New session/)).toBeInTheDocument();
  });

  it("lists sessions without a project under Sessions, apart from the projects", () => {
    setup({ projects: withLoose });
    const quick = screen.getByRole("button", { name: "Quick question" });
    expect(quick.closest(".tree-loose")).not.toBeNull();
    expect(screen.queryByRole("button", { name: "" })).not.toBeInTheDocument();
  });

  it("asks for a new project", async () => {
    const props = setup();
    await userEvent.click(screen.getByRole("button", { name: "New project" }));
    expect(props.onNewProject).toHaveBeenCalled();
  });

  it("drags a session into a project", () => {
    const props = setup({ projects: withLoose });
    const row = screen.getByRole("button", { name: "Quick question" }).parentElement!;
    const globex = screen.getByRole("button", { name: "Globex" }).closest(".tree-project")!;
    const dataTransfer = { setData: vi.fn(), effectAllowed: "", dropEffect: "" };
    fireEvent.dragStart(row, { dataTransfer });
    fireEvent.dragOver(globex, { dataTransfer });
    expect(globex).toHaveAttribute("data-drop", "true");
    fireEvent.drop(globex, { dataTransfer });
    expect(props.onMoveSession).toHaveBeenCalledWith({ project: "", sessionId: "s3" }, "Globex");
  });

  it("drags a session out of its project, and not into the one it is in", () => {
    const props = setup({ projects: withLoose });
    const row = screen.getByRole("button", { name: "Invoice layout" }).parentElement!;
    const acme = screen.getByRole("button", { name: "Acme" }).closest(".tree-project")!;
    const dataTransfer = { setData: vi.fn(), effectAllowed: "", dropEffect: "" };
    fireEvent.dragStart(row, { dataTransfer });
    fireEvent.drop(acme, { dataTransfer });
    expect(props.onMoveSession).not.toHaveBeenCalled();
    fireEvent.dragStart(row, { dataTransfer });
    fireEvent.drop(document.querySelector(".tree-loose")!, { dataTransfer });
    expect(props.onMoveSession).toHaveBeenCalledWith({ project: "Acme", sessionId: "s2" }, "");
  });

  it("lists five sessions per project, with Show more for the rest, and the open one always", async () => {
    const many = Array.from({ length: 8 }, (_, i) => session(`m${i}`, `Spec ${i}`));
    setup({ projects: [{ name: "Big", sessions: many }], selected: { project: "Big", sessionId: "m7" } });
    expect(screen.getByRole("button", { name: "Spec 4" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Spec 5" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Spec 7" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Show 2 more" }));
    expect(screen.getByRole("button", { name: "Spec 5" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Show less" }));
    expect(screen.queryByRole("button", { name: "Spec 5" })).not.toBeInTheDocument();
  });

  it("unfolds a closed project when it is picked in the search", async () => {
    const props: SidebarProps = {
      projects,
      selected: null,
      onSelect: vi.fn(),
      onNewSession: vi.fn(),
      onSearch: vi.fn(),
      onNewProject: vi.fn(),
      onCreateSession: vi.fn(),
      onDeleteSession: vi.fn(),
      onDeleteProject: vi.fn(),
      onMoveSession: vi.fn(),
      onPinSession: vi.fn(),
      onPinProject: vi.fn(),
      onRenameSession: vi.fn(),
      onOpenSettings: vi.fn(),
    };
    const { rerender } = render(<Sidebar {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "Acme" }));
    expect(screen.queryByRole("button", { name: "Invoice layout" })).not.toBeInTheDocument();
    rerender(<Sidebar {...props} reveal={{ project: "Acme", at: 1 }} />);
    expect(await screen.findByRole("button", { name: "Invoice layout" })).toBeInTheDocument();
  });

  it("pins a session or a project, and lists what is pinned on top", async () => {
    const props = setup();
    await userEvent.click(screen.getByRole("button", { name: "Pin Invoice layout" }));
    expect(props.onPinSession).toHaveBeenCalledWith({ project: "Acme", sessionId: "s2" }, true);
    await userEvent.click(screen.getByRole("button", { name: "Pin project Globex" }));
    expect(props.onPinProject).toHaveBeenCalledWith("Globex", true);
  });

  it("shows pinned projects and sessions in Pinned, and unpins them there", async () => {
    const pinned: ProjectInfo[] = [
      {
        name: "Acme",
        sessions: [session("s1", "Delivery window"), { ...session("s2", "Invoice layout"), pinned: true }],
      },
      { name: "Globex", pinned: true, sessions: [session("g1", "Lots")] },
    ];
    const props = setup({ projects: pinned });
    const group = screen.getByRole("group", { name: "Pinned" });
    expect(within(group).getByRole("button", { name: "Globex" })).toBeInTheDocument();
    expect(within(group).getByRole("button", { name: "Invoice layout" }).parentElement).toHaveTextContent(
      "Acme",
    );
    // A pinned project leaves Projects; a pinned session stays in its project too.
    expect(screen.getAllByRole("button", { name: "Globex" })).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: "Invoice layout" })).toHaveLength(2);
    await userEvent.click(within(group).getByRole("button", { name: "Unpin project Globex" }));
    expect(props.onPinProject).toHaveBeenCalledWith("Globex", false);
  });
});
