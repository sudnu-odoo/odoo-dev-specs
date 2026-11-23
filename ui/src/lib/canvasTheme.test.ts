import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import {
  getCanvasTheme,
  setCanvasTheme,
  useAppTheme,
  useCanvasTheme,
  useResolvedCanvasTheme,
} from "./canvasTheme";

beforeEach(() => localStorage.clear());

describe("canvas theme", () => {
  it("is light until chosen otherwise", () => {
    expect(getCanvasTheme()).toBe("light");
    localStorage.setItem("canvasTheme", "nonsense");
    expect(getCanvasTheme()).toBe("light");
  });

  it("remembers the choice and updates open canvases at once", () => {
    const { result } = renderHook(() => useCanvasTheme());
    act(() => setCanvasTheme("dark"));
    expect(result.current).toBe("dark");
    expect(getCanvasTheme()).toBe("dark");
  });

  it("resolves System to the system's theme", () => {
    setCanvasTheme("system");
    const { result } = renderHook(() => useResolvedCanvasTheme());
    // jsdom has no matchMedia: it counts as a light system.
    expect(result.current).toBe("light");
  });

  it("applies to the whole app, not only the diagram", () => {
    renderHook(() => useAppTheme());
    expect(document.documentElement.dataset.theme).toBe("light");
    act(() => setCanvasTheme("dark"));
    expect(document.documentElement.dataset.theme).toBe("dark");
    act(() => setCanvasTheme("system"));
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});
