import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../lib/api";
import VersionsPage from "./VersionsPage";

vi.mock("../lib/api", () => ({
  api: {
    odooVersions: vi.fn(),
    odooPublishedVersions: vi.fn(),
    onSetupProgress: vi.fn(async (handler: Progress) => {
      progress = handler;
      return () => {};
    }),
    updateOdooVersion: vi.fn(),
    removeOdooVersion: vi.fn(),
    downloadOdoo: vi.fn(async () => {}),
    useOdooFolder: vi.fn(),
    githubStatus: vi.fn(async () => ({ installed: true, connected: false, enterprise: false })),
    githubConnect: vi.fn(),
  },
}));
vi.mock("../lib/confirm", () => ({ confirmAction: vi.fn(async () => true) }));
const folders: (string | null)[] = [];
vi.mock("../lib/pickFolder", () => ({ pickFolder: vi.fn(async () => folders.shift() ?? null) }));
type Progress = (u: {
  key: string;
  task: { state: string; progress: number | null; message: string };
}) => void;
let progress: Progress = () => {};
const mocked = vi.mocked(api);
const ready = { state: "ready", linked: false } as const;

beforeEach(() => {
  vi.clearAllMocks();
  mocked.odooVersions.mockResolvedValue({
    downloadsFolder: "/home/me/odoo-versions",
    downloaded: [
      {
        version: "saas-19.2",
        community: ready,
        enterprise: ready,
        updated: Date.now(),
        sizeBytes: 2.8 * 1024 ** 3,
        communityPath: "/home/me/odoo-versions/saas-19.2/odoo",
        enterprisePath: "/home/me/odoo-versions/saas-19.2/enterprise",
      },
      {
        version: "19.0",
        community: { state: "ready", linked: true },
        enterprise: { state: "missing" },
        updated: 0,
        sizeBytes: 1024 ** 2,
        communityPath: "/home/me/odev/worktrees/19.0/odoo",
        enterprisePath: null,
      },
    ],
  });
  mocked.odooPublishedVersions.mockResolvedValue({
    available: ["saas-19.2", "19.0", "18.0"],
    latestSaas: "saas-19.2",
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

  it("shows the versions here at once, while GitHub's list is still loading", async () => {
    mocked.odooPublishedVersions.mockReturnValue(new Promise(() => {}));
    render(<VersionsPage />);
    expect(await screen.findByRole("group", { name: "Odoo 19.2" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Loading the versions Odoo publishes…");
  });

  it("says when GitHub cannot be reached", async () => {
    mocked.odooPublishedVersions.mockRejectedValue("GitHub did not answer within 20 s.");
    render(<VersionsPage />);
    expect(await screen.findByText(/Could not get the versions from GitHub/)).toHaveTextContent(
      "GitHub did not answer within 20 s.",
    );
  });

  it("shows where each version's files are, and where downloads go", async () => {
    render(<VersionsPage />);
    const saas = await screen.findByRole("group", { name: "Odoo 19.2" });
    expect(saas).toHaveTextContent("/home/me/odoo-versions/saas-19.2");
    expect(screen.getByRole("group", { name: "Odoo 19.0" })).toHaveTextContent(
      "/home/me/odev/worktrees/19.0/odoo",
    );
    expect(screen.getByText(/Downloads go to/)).toHaveTextContent(
      "/home/me/odoo-versions/<version>/odoo (Community) and /enterprise (Enterprise)",
    );
  });

  it("downloads Community then Enterprise, showing each one's progress until it is ready", async () => {
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
    // The row stays, with what is happening.
    const row = screen.getByRole("group", { name: "Odoo 18.0" });
    expect(row).toHaveTextContent("Community: starting…");
    act(() =>
      progress({ key: "community:18.0", task: { state: "running", progress: 45, message: "Downloading…" } }),
    );
    expect(row).toHaveTextContent("Community: downloading… 45%");
    act(() =>
      progress({ key: "community:18.0", task: { state: "done", progress: null, message: "Ready." } }),
    );
    await waitFor(() => expect(mocked.downloadOdoo).toHaveBeenCalledWith("18.0", "enterprise"));
    expect(row).toHaveTextContent("Community: ready");
    act(() =>
      progress({
        key: "enterprise:18.0",
        task: { state: "noAccess", progress: null, message: "No access." },
      }),
    );
    await waitFor(() =>
      expect(screen.getByRole("group", { name: "Odoo 18.0" })).toHaveTextContent(
        "Enterprise: no access (needs a GitHub account with Odoo Enterprise access)",
      ),
    );
  });

  it("uses folders the person already has, for a version here or a new one", async () => {
    render(<VersionsPage />);
    const saas = await screen.findByRole("group", { name: "Odoo 19.2" });
    folders.push("/home/me/odev/worktrees/saas-19.2/odoo", "/home/me/odev/worktrees/saas-19.2/enterprise");
    await userEvent.click(within(saas).getByRole("button", { name: "Use a folder…" }));
    expect(mocked.useOdooFolder).toHaveBeenCalledWith(
      "saas-19.2",
      "community",
      "/home/me/odev/worktrees/saas-19.2/odoo",
    );
    expect(mocked.useOdooFolder).toHaveBeenCalledWith(
      "saas-19.2",
      "enterprise",
      "/home/me/odev/worktrees/saas-19.2/enterprise",
    );
    // Already a folder of the person's own: nothing to choose there.
    const own = screen.getByRole("group", { name: "Odoo 19.0" });
    expect(within(own).queryByRole("button", { name: "Use a folder…" })).not.toBeInTheDocument();

    folders.push("/home/me/odoo-18", null);
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Version to add" }), "18.0");
    const add = screen.getByRole("button", { name: "Download" }).parentElement!;
    await userEvent.click(within(add).getByRole("button", { name: "Use a folder…" }));
    expect(mocked.useOdooFolder).toHaveBeenCalledWith("18.0", "community", "/home/me/odoo-18");
  });

  it("offers to connect GitHub, only for downloading", async () => {
    render(<VersionsPage />);
    const github = await screen.findByRole("group", { name: "GitHub" });
    expect(github).toHaveTextContent("only needed to download versions");
    await userEvent.click(await within(github).findByRole("button", { name: "Connect GitHub" }));
    expect(mocked.githubConnect).toHaveBeenCalled();
  });
});
