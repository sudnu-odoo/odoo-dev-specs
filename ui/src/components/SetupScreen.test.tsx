import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Setup } from "../app/useSetup";
import type { SetupStatus, SetupTask } from "../lib/types";
import SetupScreen from "./SetupScreen";

const ready: SetupStatus = {
  version: "19.0",
  system: { requirements: [], ready: true },
  community: { state: "ready", linked: false },
  enterprise: { state: "missing" },
  github: { installed: true, connected: true, enterprise: false },
  aiReady: true,
  tasks: {},
};

function setup(
  status: Partial<SetupStatus>,
  tasks: { community?: SetupTask; enterprise?: SetupTask; tools?: SetupTask } = {},
  complete = false,
) {
  const s = {
    version: "19.0",
    status: { ...ready, ...status },
    error: null,
    dismissError: vi.fn(),
    refresh: vi.fn(),
    complete,
    downloading: null,
    ...tasks,
    setUpEverything: vi.fn(),
    installTools: vi.fn(),
    download: vi.fn(),
    chooseFolder: vi.fn(),
    githubConnect: vi.fn(),
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
  it("shows a download with its progress", () => {
    setup(
      { community: { state: "missing" } },
      { community: { state: "running", progress: 45, message: "Downloading…" } },
    );
    expect(row("Odoo 19 Community")).toHaveTextContent("Downloading… 45%");
    expect(within(row("Odoo 19 Community")).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "45");
  });

  it("explains Python setup after the download, without a fake percentage", () => {
    setup(
      { community: { state: "missing" } },
      { community: { state: "running", progress: null, message: "Preparing Odoo… (a few minutes)" } },
    );
    expect(row("Odoo 19 Community")).toHaveTextContent("Preparing Odoo");
    expect(within(row("Odoo 19 Community")).getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
  });

  it("connects GitHub with one button, and Odoo waits for it", async () => {
    const { s } = setup({
      github: { installed: true, connected: false, enterprise: false },
      community: { state: "missing" },
    });
    expect(row("GitHub")).toHaveTextContent(/one sign-in in your browser/i);
    expect(row("Odoo 19 Community")).toHaveTextContent(/once GitHub is connected/);
    await userEvent.click(within(row("GitHub")).getByRole("button", { name: "Connect GitHub" }));
    expect(s.githubConnect).toHaveBeenCalled();
  });

  it("says when the GitHub account cannot read Enterprise yet, with Try again", async () => {
    const { s } = setup({ github: { installed: true, connected: true, enterprise: false } });
    expect(row("GitHub")).toHaveAttribute("data-state", "done");
    expect(row("Odoo 19 Enterprise")).toHaveTextContent(/Not available yet/);
    expect(row("Odoo 19 Enterprise")).toHaveAttribute("data-state", "optional");
    await userEvent.click(within(row("Odoo 19 Enterprise")).getByRole("button", { name: "Try again" }));
    expect(s.download).toHaveBeenCalledWith("enterprise");
  });

  it("offers the logins of the AI services that are installed", async () => {
    const { onLogin } = setup({ aiReady: false });
    const ai = row("AI assistant");
    expect(within(ai).queryByRole("button", { name: /Antigravity/ })).not.toBeInTheDocument();
    await userEvent.click(within(ai).getByRole("button", { name: "Log in to Claude" }));
    expect(onLogin).toHaveBeenCalledWith("claude");
  });

  it("shows a failed install with its reason, and runs everything from one button", async () => {
    const { s } = setup(
      { system: { ready: false, requirements: [{ key: "git", label: "Git", ok: false, problem: "" }] } },
      { tools: { state: "failed", progress: null, message: "The password was not given." } },
    );
    expect(row("Tools")).toHaveTextContent("The password was not given.");
    await userEvent.click(screen.getByRole("button", { name: "Set up everything" }));
    expect(s.setUpEverything).toHaveBeenCalled();
  });

  it("ends with one button into the app", async () => {
    const { onDone } = setup({}, {}, true);
    expect(screen.getByText("Everything is ready.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Start writing specs" }));
    expect(onDone).toHaveBeenCalled();
  });
});
