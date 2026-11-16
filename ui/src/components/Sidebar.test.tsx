import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import Sidebar, { type SidebarProps } from "./Sidebar";
import type { ProjectInfo } from "../lib/types";

const session = (id: string, title: string) => ({
  id,
  title,
  provider: "claude",
  created_at: 0,
  updated_at: 0,
  started: false,
});

const projects: ProjectInfo[] = [
  { name: "Acme", sessions: [session("s1", "Delivery window"), session("s2", "Invoice layout")] },
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
    ...overrides,
  };
  render(<Sidebar {...props} />);
  return props;
}

describe("Sidebar", () => {
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
