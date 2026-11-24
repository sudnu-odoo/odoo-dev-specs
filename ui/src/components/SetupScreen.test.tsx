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
  pythonReady: true,
});
const ready: SetupStatus = {
  system: {
    requirements: [
      { key: "postgresql", label: "PostgreSQL", ok: true, problem: "", cause: null, help: "", fix: null },
      {
        key: "pdf",
        label: "PDF tools",
        ok: false,
        problem: "The PDF tools are not installed.",
        cause: "pdfToolsMissing",
        help: "They read the reports attached in the chat.",
        fix: "automatic",
      },
    ],
    ready: true,
  },
  versions: ["19.0"],
  sources: [source("19.0", "/home/me/odev/worktrees/19.0")],
  broken: [],
  found: [],
  aiReady: true,
  tasks: {},
};
const found = (version: string) => ({ version, community: `/h/${version}/odoo`, enterprise: null });

function setup(
  status: Partial<SetupStatus>,
  tools?: SetupTask,
  complete = false,
  github?: { installed: boolean; connected: boolean; enterprise: boolean; unreachable: boolean },
) {
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
    github: github ?? { installed: true, connected: true, enterprise: true, unreachable: false },
    repairPostgres: vi.fn(),
    forgetSource: vi.fn(),
    prepare: vi.fn(),
    chooseAgain: vi.fn(),
    copyReport: vi.fn(async () => true),
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
  it("GitHub is optional: not connected never blocks the setup, and nothing is downloaded", () => {
    setup({ versions: [], found: [found("19.0")] }, undefined, false, {
      installed: true,
      connected: false,
      enterprise: false,
      unreachable: false,
    });
    expect(row("GitHub")).toHaveTextContent("Optional");
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
      {
        system: {
          ready: false,
          requirements: [
            {
              key: "git",
              label: "Git",
              ok: false,
              problem: "",
              cause: "gitMissing",
              help: "",
              fix: "automatic",
            },
          ],
        },
      },
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

  it("explains each problem: why it matters, and what the person does when the app cannot", async () => {
    setup({
      system: {
        ready: false,
        requirements: [
          {
            key: "postgresql",
            label: "PostgreSQL",
            ok: false,
            problem: "Another program uses PostgreSQL's port.",
            cause: "postgresPortTaken",
            help: "Port 5432 is taken, often by PostgreSQL running in Docker.",
            fix: "manual",
          },
        ],
      },
    });
    await userEvent.click(screen.getByRole("button", { name: "Tools" }));
    const details = within(row("Tools")).getByRole("list", { name: "Tools details" });
    expect(details).toHaveTextContent("Another program uses PostgreSQL's port.");
    expect(details).toHaveTextContent("Port 5432 is taken");
    expect(details).toHaveTextContent("You fix this one");
    // Reinstalling cannot free a port another program holds.
    expect(within(details).queryByRole("button", { name: "Repair PostgreSQL" })).not.toBeInTheDocument();
  });

  it("offers to repair PostgreSQL when installing did not cure it", async () => {
    const { s } = setup(
      {
        system: {
          ready: false,
          requirements: [
            {
              key: "postgresql",
              label: "PostgreSQL",
              ok: false,
              problem: "PostgreSQL is installed but stopped.",
              cause: "postgresStopped",
              help: "",
              fix: "automatic",
            },
          ],
        },
      },
      { state: "failed", progress: null, message: "PostgreSQL is installed but stopped." },
    );
    await userEvent.click(screen.getByRole("button", { name: "Tools" }));
    await userEvent.click(screen.getByRole("button", { name: "Repair PostgreSQL" }));
    expect(s.repairPostgres).toHaveBeenCalled();
  });

  it("shows a moved Odoo folder, to choose again or forget", async () => {
    const { s } = setup({
      versions: [],
      sources: [],
      broken: [{ version: "19.0", edition: "community", path: "/home/me/old/odoo" }],
    });
    expect(row("Odoo source")).toHaveAttribute("data-state", "attention");
    expect(row("Odoo source")).toHaveTextContent("19.0 Community: its folder was moved or deleted");
    await userEvent.click(within(row("Odoo source")).getByRole("button", { name: "Choose again" }));
    expect(s.chooseAgain).toHaveBeenCalledWith("19.0", "community");
    await userEvent.click(within(row("Odoo source")).getByRole("button", { name: "Forget it" }));
    expect(s.forgetSource).toHaveBeenCalledWith("19.0", "community");
  });

  it("prepares a version's Python packages, and says why it failed", async () => {
    const notReady = { ...source("19.0", "/home/me/odev/worktrees/19.0"), pythonReady: false };
    const { s } = setup({
      sources: [notReady],
      tasks: {
        "python:19.0": { state: "failed", progress: null, message: "No matching distribution for psycopg2" },
      },
    });
    await userEvent.click(screen.getByRole("button", { name: "Odoo source" }));
    const details = within(row("Odoo source")).getByRole("list", { name: "Odoo source details" });
    expect(details).toHaveTextContent("Python packages failed: No matching distribution for psycopg2");
    await userEvent.click(within(details).getByRole("button", { name: "Try again" }));
    expect(s.prepare).toHaveBeenCalledWith("19.0");
  });

  it("says when GitHub does not answer, apart from not being connected", async () => {
    setup({}, undefined, false, { installed: true, connected: false, enterprise: false, unreachable: true });
    expect(row("GitHub")).toHaveTextContent("GitHub does not answer");
  });

  it("checks again, and copies a report for support", async () => {
    const { s } = setup({});
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(s.refresh).toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Copy a report for support" }));
    expect(s.copyReport).toHaveBeenCalled();
    expect(await screen.findByText("Copied")).toBeInTheDocument();
  });
});
