import { act, renderHook } from "@testing-library/react";
import { fireEvent } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { useSpecPanel } from "./useSpecPanel";

beforeEach(() => localStorage.clear());

describe("useSpecPanel", () => {
  it("is hidden at first, toggles with Ctrl+B, and is remembered", () => {
    const { result, unmount } = renderHook(() => useSpecPanel());
    expect(result.current.open).toBe(false);
    act(() => {
      fireEvent.keyDown(window, { key: "b", ctrlKey: true });
    });
    expect(result.current.open).toBe(true);
    unmount();
    const again = renderHook(() => useSpecPanel());
    expect(again.result.current.open).toBe(true);
    act(() => again.result.current.toggle());
    expect(again.result.current.open).toBe(false);
  });

  it("leaves plain b and other shortcuts alone", () => {
    const { result } = renderHook(() => useSpecPanel());
    act(() => {
      fireEvent.keyDown(window, { key: "b" });
      fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    });
    expect(result.current.open).toBe(false);
  });
});
