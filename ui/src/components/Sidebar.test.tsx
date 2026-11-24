import { render, screen, within } from "@testing-library/react";
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

function setup(overrides: Partial<SidebarProps> = {}) {
  const props: SidebarProps = {
    projects,
    selected: { project: "Acme", sessionId: "s1" },
    onSelect: vi.fn(),
    onCreateProject: vi.fn().mockResolvedValue(undefined),
    onCreateSession: vi.fn(),
    onDeleteSession: vi.fn(),
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
    expect(screen.getByRole("button", { name: "Delivery window" })).toHaveTextContent("Online · 19.2");
    const invoice = screen.getByRole("button", { name: "Invoice layout" });
    expect(within(invoice).getByRole("status", { name: "Working" })).toBeInTheDocument();
  });

  it("finds sessions by their title", async () => {
    setup();
    await userEvent.type(screen.getByRole("searchbox", { name: "Search sessions" }), "invoice");
    expect(screen.getByRole("button", { name: "Invoice layout" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delivery window" })).not.toBeInTheDocument();
    expect(screen.queryByText("Globex")).not.toBeInTheDocument();
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

  it("creates a project from the inline form", async () => {
    const props = setup();
    await userEvent.click(screen.getByRole("button", { name: "New project" }));
    await userEvent.type(screen.getByPlaceholderText("Project name"), "Initech{Enter}");
    expect(props.onCreateProject).toHaveBeenCalledWith("Initech");
    expect(screen.queryByPlaceholderText("Project name")).not.toBeInTheDocument();
  });

  it("keeps the form open and shows the error when creation fails", async () => {
    setup({ onCreateProject: vi.fn().mockRejectedValue('A project named "Acme" already exists.') });
    await userEvent.click(screen.getByRole("button", { name: "New project" }));
    await userEvent.type(screen.getByPlaceholderText("Project name"), "Acme{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("already exists");
    expect(screen.getByPlaceholderText("Project name")).toBeInTheDocument();
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

  it("guides the user when there are no projects yet", () => {
    setup({ projects: [], selected: null });
    expect(screen.getByText(/create a project/i)).toBeInTheDocument();
  });
});
