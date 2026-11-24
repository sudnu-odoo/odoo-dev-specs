import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Setup } from "../app/useSetup";
import type { SetupStatus, SetupTask } from "../lib/types";
import SetupScreen from "./SetupScreen";

vi.mock("../lib/pickFolder", () => ({ pickFolder: vi.fn(async () => "/home/me/odev/worktrees/19.0") }));

const source = (version: string, path: string) => ({
  version,
  community: { state: "ready" as const, linked: true },
  enterprise: { state: "ready" as const, linked: true },
  updated: 0,
  sizeBytes: 0,
  communityPath: `${path}/odoo`,
  enterprisePath: `${path}/enterprise`,
});
const ready: SetupStatus = {
  system: {
    requirements: [
      { key: "postgresql", label: "PostgreSQL", ok: true, problem: "" },
      { key: "pdf", label: "PDF tools", ok: false, problem: "The PDF tools are not installed." },
    ],
    ready: true,
  },
  versions: ["19.0"],
  sources: [source("19.0", "/home/me/odev/worktrees/19.0")],
  found: [],
  aiReady: true,
  tasks: {},
};
const found = (version: string) => ({ version, community: `/h/${version}/odoo`, enterprise: null });

function setup(status: Partial<SetupStatus>, tools?: SetupTask, complete = false) {
  const s = {
    status: { ...ready, ...status },
    error: null,
    dismissError: vi.fn(),
    refresh: vi.fn(),
    complete,
    downloading: null,
    tools,
    setUpEverything: vi.fn(),
    useFound: vi.fn(),
    installTools: vi.fn(),
    chooseFolder: vi.fn(),
  } as unknown as Setup;
  const props = { onLogin: vi.fn(), onDone: vi.fn() };
  render(
    <SetupScreen
      setup={s}
      providers={[
        { provider: "claude", name: "Claude", installed: true, loggedIn: false },
        { provider: "antigravity", name: "Antigravity", installed: false, loggedIn: false },
      ]}
      {...props}
    />,
  );
  return { s, ...props };
}
const row = (name: string) => screen.getByRole("listitem", { name });

describe("SetupScreen", () => {
  it("has no GitHub step and downloads nothing", () => {
    setup({ versions: [], found: [found("19.0")] });
    expect(screen.queryByRole("listitem", { name: "GitHub" })).not.toBeInTheDocument();
    expect(row("Odoo source")).toHaveTextContent("nothing is downloaded");
  });

  it("shows the Odoo found on the computer, and uses it with Set up everything", async () => {
    const { s } = setup({
      versions: [],
      found: ["saas-19.2", "19.0", "18.0", "17.0", "saas-18.4"].map(found),
    });
    expect(row("Odoo source")).toHaveTextContent(
      "Found on this computer: Odoo 19.2, 19.0, 18.0, 17.0 and 1 more",
    );
    await userEvent.click(screen.getByRole("button", { name: "Set up everything" }));
    expect(s.setUpEverything).toHaveBeenCalled();
  });

  it("counts the source as ready when a version is in use, other folders found are extras", async () => {
    const { s } = setup({
      versions: ["19.0", "20.0"],
      found: ["saas-19.4", "saas-19.2", "18.0"].map(found),
    });
    expect(row("Odoo source")).toHaveAttribute("data-state", "done");
    expect(row("Odoo source")).toHaveTextContent("Using Odoo 19.0, 20.0 from your folders.");
    expect(row("Odoo source")).toHaveTextContent("Also found: 19.4, 19.2, 18.0");
    await userEvent.click(within(row("Odoo source")).getByRole("button", { name: "Use them too" }));
    expect(s.useFound).toHaveBeenCalled();
  });

  it("opens each step to show what is installed and where", async () => {
    setup({});
    await userEvent.click(screen.getByRole("button", { name: "Tools" }));
    const tools = within(row("Tools")).getByRole("list", { name: "Tools details" });
    expect(tools).toHaveTextContent("PostgreSQL");
    expect(tools).toHaveTextContent("The PDF tools are not installed.");
    await userEvent.click(screen.getByRole("button", { name: "Odoo source" }));
    expect(within(row("Odoo source")).getByRole("list", { name: "Odoo source details" })).toHaveTextContent(
      "19.0Community + Enterprise/home/me/odev/worktrees/19.0",
    );
    await userEvent.click(screen.getByRole("button", { name: "AI assistant" }));
    const ai = within(row("AI assistant")).getByRole("list", { name: "AI assistant details" });
    expect(ai).toHaveTextContent("ClaudeInstalled, not logged in");
    expect(ai).toHaveTextContent("AntigravityNot installed");
    await userEvent.click(screen.getByRole("button", { name: "Tools" }));
    expect(within(row("Tools")).queryByRole("list")).not.toBeInTheDocument();
  });

  it("says which versions are in use", () => {
    setup({ versions: ["saas-19.2", "19.0"] });
    expect(row("Odoo source")).toHaveTextContent("Using Odoo 19.2, 19.0 from your folders.");
    expect(row("Odoo source")).toHaveAttribute("data-state", "done");
  });

  it("asks for the folder when no Odoo is found", async () => {
    const { s } = setup({ versions: [], found: [] });
    expect(row("Odoo source")).toHaveTextContent("No Odoo source found");
    await userEvent.click(within(row("Odoo source")).getByRole("button", { name: "Choose a folder…" }));
    expect(s.chooseFolder).toHaveBeenCalledWith("/home/me/odev/worktrees/19.0");
  });

  it("offers the logins of the AI services that are installed", async () => {
    const { onLogin } = setup({ aiReady: false });
    const ai = row("AI assistant");
    expect(within(ai).queryByRole("button", { name: /Antigravity/ })).not.toBeInTheDocument();
    await userEvent.click(within(ai).getByRole("button", { name: "Log in to Claude" }));
    expect(onLogin).toHaveBeenCalledWith("claude");
  });

  it("shows a failed install with its reason", () => {
    setup(
      { system: { ready: false, requirements: [{ key: "git", label: "Git", ok: false, problem: "" }] } },
      { state: "failed", progress: null, message: "The password was not given." },
    );
    expect(row("Tools")).toHaveTextContent("The password was not given.");
  });

  it("ends with one button into the app", async () => {
    const { onDone } = setup({}, undefined, true);
    expect(screen.getByText("Everything is ready.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Start writing specs" }));
    expect(onDone).toHaveBeenCalled();
  });
});
