import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Setup } from "../app/useSetup";
import type { SetupStatus, SetupTask } from "../lib/types";
import SetupScreen from "./SetupScreen";

vi.mock("../lib/pickFolder", () => ({ pickFolder: vi.fn(async () => "/home/me/odev/worktrees/19.0") }));

const ready: SetupStatus = {
  system: { requirements: [], ready: true },
  versions: ["19.0"],
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
