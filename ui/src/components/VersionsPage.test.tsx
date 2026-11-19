import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../lib/api";
import VersionsPage from "./VersionsPage";

vi.mock("../lib/api", () => ({
  api: {
    odooVersions: vi.fn(),
    updateOdooVersion: vi.fn(),
    removeOdooVersion: vi.fn(),
    downloadOdoo: vi.fn(),
  },
}));
vi.mock("../lib/confirm", () => ({ confirmAction: vi.fn(async () => true) }));
const mocked = vi.mocked(api);
const ready = { state: "ready", linked: false } as const;

beforeEach(() => {
  vi.clearAllMocks();
  mocked.odooVersions.mockResolvedValue({
    available: ["saas-19.2", "19.0", "18.0"],
    latestSaas: "saas-19.2",
    downloaded: [
      {
        version: "saas-19.2",
        community: ready,
        enterprise: ready,
        updated: Date.now(),
        sizeBytes: 2.8 * 1024 ** 3,
      },
      {
        version: "19.0",
        community: { state: "ready", linked: true },
        enterprise: { state: "missing" },
        updated: 0,
        sizeBytes: 1024 ** 2,
      },
    ],
  });
});

describe("VersionsPage", () => {
  it("lists the versions here with their editions, size and last update", async () => {
    render(<VersionsPage />);
    const saas = await screen.findByRole("group", { name: "Odoo 19.2" });
    expect(saas).toHaveTextContent("Community + Enterprise · 2.8 GB · updated today");
    const own = screen.getByRole("group", { name: "Odoo 19.0" });
    expect(own).toHaveTextContent("your own folder");
    expect(within(own).getByRole("button", { name: "Update now" })).toBeDisabled();
  });

  it("updates and removes a version", async () => {
    render(<VersionsPage />);
    const saas = await screen.findByRole("group", { name: "Odoo 19.2" });
    await userEvent.click(within(saas).getByRole("button", { name: "Update now" }));
    expect(mocked.updateOdooVersion).toHaveBeenCalledWith("saas-19.2");
    await userEvent.click(within(saas).getByRole("button", { name: "Remove" }));
    expect(mocked.removeOdooVersion).toHaveBeenCalledWith("saas-19.2");
  });

  it("adds a version Odoo publishes that is not here yet", async () => {
    render(<VersionsPage />);
    const select = await screen.findByRole("combobox", { name: "Version to add" });
    expect(
      within(select)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["Choose…", "18.0"]);
    await userEvent.selectOptions(select, "18.0");
    await userEvent.click(screen.getByRole("button", { name: "Download" }));
    expect(mocked.downloadOdoo).toHaveBeenCalledWith("18.0", "community");
  });
});
