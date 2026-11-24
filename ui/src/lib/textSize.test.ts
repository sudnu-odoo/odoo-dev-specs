import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const setZoom = vi.fn(async () => {});
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ setZoom }) }));

import { getTextSize, setTextSize, stepTextSize, useAppTextSize, useTextSize } from "./textSize";

beforeEach(() => {
  localStorage.clear();
  setZoom.mockClear();
});

describe("text size", () => {
  it("starts at 100% and is remembered", () => {
    expect(getTextSize()).toBe(1);
    setTextSize(1.25);
    expect(getTextSize()).toBe(1.25);
  });

  it("steps up and down within its limits, and ignores nonsense", () => {
    expect(stepTextSize(1, 1)).toBe(1.1);
    expect(stepTextSize(1, -1)).toBe(0.9);
    expect(stepTextSize(1.6, 1)).toBe(1.6);
    expect(stepTextSize(0.8, -1)).toBe(0.8);
    localStorage.setItem("textSize", "7");
    expect(getTextSize()).toBe(1);
  });

  it("zooms the window, and follows changes", () => {
    const { result } = renderHook(() => {
      useAppTextSize();
      return useTextSize();
    });
    expect(setZoom).toHaveBeenLastCalledWith(1);
    act(() => setTextSize(1.25));
    expect(result.current).toBe(1.25);
    expect(setZoom).toHaveBeenLastCalledWith(1.25);
  });
});
