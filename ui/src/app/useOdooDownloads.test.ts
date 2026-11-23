import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../lib/api";
import { useOdooDownloads } from "./useOdooDownloads";

type Progress = (u: {
  key: string;
  task: { state: string; progress: number | null; message: string };
}) => void;
let progress: Progress = () => {};
vi.mock("../lib/api", () => ({
  api: {
    downloadOdoo: vi.fn(async () => {}),
    onSetupProgress: vi.fn(async (handler: Progress) => {
      progress = handler;
      return () => {};
    }),
  },
}));
const mocked = vi.mocked(api);
const send = (key: string, state: string, value: number | null = null) =>
  act(() =>
    progress({ key, task: { state, progress: value, message: state === "running" ? "Downloading…" : "" } }),
  );

beforeEach(() => vi.clearAllMocks());

describe("useOdooDownloads", () => {
  it("downloads Community, then Enterprise, then refreshes the list once", async () => {
    const reload = vi.fn(async () => {});
    const { result } = renderHook(() => useOdooDownloads(reload));
    await waitFor(() => expect(mocked.onSetupProgress).toHaveBeenCalled());
    act(() => result.current.download("18.0"));
    expect(mocked.downloadOdoo).toHaveBeenCalledWith("18.0", "community");
    expect(result.current.downloads["18.0"].community?.state).toBe("starting");
    expect(result.current.inProgress("18.0")).toBe(true);

    send("community:18.0", "running", 45);
    expect(result.current.downloads["18.0"].community?.progress).toBe(45);
    send("community:18.0", "done");
    await waitFor(() => expect(mocked.downloadOdoo).toHaveBeenCalledWith("18.0", "enterprise"));
    expect(reload).not.toHaveBeenCalled();

    send("enterprise:18.0", "noAccess");
    await waitFor(() => expect(reload).toHaveBeenCalledTimes(1));
    expect(result.current.inProgress("18.0")).toBe(false);
    send("enterprise:18.0", "noAccess");
    expect(reload).toHaveBeenCalledTimes(1);
  });

  it("stops after Community when it fails, and says why", async () => {
    mocked.downloadOdoo.mockRejectedValueOnce("GitHub refused");
    const reload = vi.fn(async () => {});
    const { result } = renderHook(() => useOdooDownloads(reload));
    act(() => result.current.download("17.0"));
    await waitFor(() => expect(result.current.downloads["17.0"].community?.state).toBe("failed"));
    expect(result.current.downloads["17.0"].community?.message).toBe("GitHub refused");
    expect(mocked.downloadOdoo).not.toHaveBeenCalledWith("17.0", "enterprise");
    await waitFor(() => expect(reload).toHaveBeenCalledTimes(1));
  });
});
