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
  it("shows the Odoo settings on the General page", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(await screen.findByLabelText("Odoo version")).toHaveValue("19.0");
    expect(screen.getByRole("radio", { name: "On" })).toHaveAttribute("aria-checked", "true");
  });

  it("saves each change at once, without a Save button", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    await userEvent.selectOptions(await screen.findByLabelText("Odoo version"), "18.0");
    expect(mocked.saveSettings).toHaveBeenLastCalledWith({ ...settings, defaultVersion: "18.0" });
    await userEvent.click(screen.getByRole("radio", { name: "Off" }));
    expect(mocked.saveSettings).toHaveBeenLastCalledWith({
      ...settings,
      defaultVersion: "18.0",
      demoData: false,
    });
    expect(screen.queryByRole("button", { name: "Save" })).not.toBeInTheDocument();
  });

  it("hides a healthy setup and shows only what needs fixing", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    await screen.findByLabelText("Odoo version");
    expect(screen.queryByText("/usr/bin/git")).not.toBeInTheDocument();
    expect(screen.queryByText("Needs attention")).not.toBeInTheDocument();
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
    expect(screen.getByText("Needs attention")).toBeInTheDocument();
    expect(screen.getByText("Chrome").closest("[data-status]")).toHaveAttribute("data-status", "missing");
  });

  it("can be closed", async () => {
    const onClose = vi.fn();
    render(<SettingsPanel onClose={onClose} />);
    await userEvent.click(await screen.findByRole("button", { name: "Close settings" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("has no usage or preferred assistant: those live in the chat", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    await screen.findByLabelText("Odoo version");
    expect(screen.queryByText(/preferred assistant/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/usage/i)).not.toBeInTheDocument();
  });

  it("lists each AI service as connected or with a login button", async () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    await userEvent.click(await screen.findByRole("button", { name: "AI services" }));
    expect(await screen.findByRole("group", { name: "Claude" })).toHaveTextContent("Connected");
    await userEvent.click(screen.getByRole("button", { name: "Log in to Antigravity" }));
    expect(mocked.loginProvider).toHaveBeenCalledWith("antigravity");
  });

  it("chooses the diagram theme, light by default", async () => {
    localStorage.clear();
    render(<SettingsPanel onClose={vi.fn()} />);
    const theme = await screen.findByRole("radiogroup", { name: "Diagram theme" });
    expect(theme.querySelector('[aria-checked="true"]')).toHaveTextContent("Light");
    await userEvent.click(screen.getByRole("radio", { name: "Dark" }));
    expect(localStorage.getItem("canvasTheme")).toBe("dark");
    expect(screen.getByText(/keep their real colours/)).toBeInTheDocument();
  });
});
