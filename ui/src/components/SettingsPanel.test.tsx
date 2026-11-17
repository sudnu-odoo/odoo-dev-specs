import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPanel from "./SettingsPanel";
import { api } from "../lib/api";
import type { EnvReport, Settings } from "../lib/types";

vi.mock("../lib/api", () => ({
  api: {
    getSettings: vi.fn(),
    saveSettings: vi.fn(),
    checkEnvironment: vi.fn(),
    providerStatus: vi.fn(),
    loginProvider: vi.fn(),
  },
}));

const mocked = vi.mocked(api);
const settings: Settings = { provider: "claude", defaultVersion: "19.0", demoData: true };
const healthy: EnvReport = {
  checks: [
    { key: "git", label: "Git", status: "ok", detail: "/usr/bin/git", hint: null, required: true },
    { key: "ai", label: "AI assistant", status: "ok", detail: "Claude Code", hint: null, required: true },
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  mocked.getSettings.mockResolvedValue(settings);
  mocked.saveSettings.mockResolvedValue(undefined);
  mocked.checkEnvironment.mockResolvedValue(healthy);
  mocked.providerStatus.mockResolvedValue([
    { provider: "claude", name: "Claude", installed: true, loggedIn: true },
    { provider: "antigravity", name: "Antigravity", installed: true, loggedIn: false },
  ]);
  mocked.loginProvider.mockResolvedValue(undefined);
});

describe("SettingsPanel", () => {
  it("shows the environment checks and current settings", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(await screen.findByText("/usr/bin/git")).toBeInTheDocument();
    // The AI check reports which CLI is installed, in its detail line.
    expect(screen.getByText("AI assistant").closest(".env-row")).toHaveTextContent("Claude Code");
    expect(await screen.findByLabelText("Default Odoo version")).toHaveValue("19.0");
    expect(screen.getByLabelText(/demo data/i)).toBeChecked();
  });

  it("saves an edited setting", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    await screen.findByLabelText("Default Odoo version");
    await userEvent.clear(screen.getByLabelText("Default Odoo version"));
    await userEvent.type(screen.getByLabelText("Default Odoo version"), "18.0");
    await userEvent.click(screen.getByLabelText(/demo data/i));
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(mocked.saveSettings).toHaveBeenCalledWith({
      provider: "claude",
      defaultVersion: "18.0",
      demoData: false,
    });
  });

  it("shows a fix hint for a failing check", async () => {
    mocked.checkEnvironment.mockResolvedValue({
      checks: [
        {
          key: "chrome",
          label: "Chrome",
          status: "missing",
          detail: "not found",
          hint: "Install Google Chrome or Chromium.",
          required: true,
        },
      ],
    });
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(await screen.findByText("Install Google Chrome or Chromium.")).toBeInTheDocument();
    const row = screen.getByText("Chrome").closest("[data-status]");
    expect(row).toHaveAttribute("data-status", "missing");
  });

  it("can be closed", async () => {
    const onClose = vi.fn();
    render(<SettingsPanel onClose={onClose} />);
    await userEvent.click(await screen.findByRole("button", { name: "Close settings" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("no longer asks for a preferred assistant: the model menu decides", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    await screen.findByLabelText("Default Odoo version");
    expect(screen.queryByText(/preferred assistant/i)).not.toBeInTheDocument();
  });

  it("lists each AI assistant with its login status and a login button", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    const claude = await screen.findByRole("listitem", { name: "Claude" });
    expect(claude).toHaveTextContent("Logged in");
    const agy = screen.getByRole("listitem", { name: "Antigravity" });
    expect(agy).toHaveTextContent("Not logged in");
    await userEvent.click(screen.getByRole("button", { name: "Log in to Antigravity" }));
    expect(mocked.loginProvider).toHaveBeenCalledWith("antigravity");
  });

  it("shows plan usage once the assistant has reported it", async () => {
    const now = Date.now() / 1000;
    render(
      <SettingsPanel
        onClose={vi.fn()}
        plan={{ five_hour: { utilization: 0.3, resets_at: now + 3600 }, seven_day: null }}
      />,
    );
    const usage = await screen.findByRole("region", { name: "Usage" });
    expect(usage).toHaveTextContent("30%");
  });

  it("explains when usage will appear", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    const usage = await screen.findByRole("region", { name: "Usage" });
    expect(usage).toHaveTextContent(/after the first reply/i);
  });
});
