import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../lib/api";
import UpdateBanner from "./UpdateBanner";

vi.mock("../lib/api", () => ({ api: { checkUpdate: vi.fn(), installUpdate: vi.fn() } }));
const mocked = vi.mocked(api);
const RELEASE = { version: "0.2.0", notes: "- Pick the database per session\n- Faster setup", debUrl: "x" };

beforeEach(() => vi.clearAllMocks());

describe("UpdateBanner", () => {
  it("stays hidden when the app is up to date", async () => {
    mocked.checkUpdate.mockResolvedValue(null);
    render(<UpdateBanner />);
    await Promise.resolve();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("offers a newer version with what changed, and installs it", async () => {
    mocked.checkUpdate.mockResolvedValue(RELEASE);
    mocked.installUpdate.mockReturnValue(new Promise(() => {}));
    render(<UpdateBanner />);
    expect(await screen.findByText("Version 0.2.0 is ready.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "What's new" }));
    expect(screen.getAllByRole("listitem").map((l) => l.textContent)).toEqual([
      "Pick the database per session",
      "Faster setup",
    ]);
    await userEvent.click(screen.getByRole("button", { name: "Update now" }));
    expect(mocked.installUpdate).toHaveBeenCalledWith(RELEASE);
    expect(screen.getByText("Installing the update…")).toBeInTheDocument();
  });

  it("says why an update failed and can try again, or wait until later", async () => {
    mocked.checkUpdate.mockResolvedValue(RELEASE);
    mocked.installUpdate.mockRejectedValue(
      "The update was not installed (the password was not given, or it failed).",
    );
    render(<UpdateBanner />);
    await userEvent.click(await screen.findByRole("button", { name: "Update now" }));
    expect(await screen.findByText(/password was not given/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Try again" })).toBeEnabled();
    await userEvent.click(screen.getByRole("button", { name: "Later" }));
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("is quiet when GitHub cannot be reached", async () => {
    mocked.checkUpdate.mockRejectedValue("offline");
    render(<UpdateBanner />);
    await Promise.resolve();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});
