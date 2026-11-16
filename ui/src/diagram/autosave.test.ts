import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Autosave } from "./autosave";

let saved: string[];
let auto: Autosave;

beforeEach(() => {
  vi.useFakeTimers();
  saved = [];
  auto = new Autosave((s) => saved.push(s), 800, "disk");
});
afterEach(() => vi.useRealTimers());

describe("Autosave", () => {
  it("saves a real change once, after a pause", () => {
    auto.change("a", () => "A");
    auto.change("b", () => "B");
    vi.advanceTimersByTime(799);
    expect(saved).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(saved).toEqual(["B"]);
  });

  it("does not save what is already on disk", () => {
    auto.change("disk", () => "same");
    vi.advanceTimersByTime(2000);
    expect(saved).toEqual([]);
  });

  it("cancels a pending save when the canvas returns to the disk content", () => {
    // E.g. a passing empty canvas before the editor loaded the drawing.
    auto.change("empty", () => "EMPTY");
    auto.change("disk", () => "same");
    vi.advanceTimersByTime(2000);
    expect(saved).toEqual([]);
  });

  it("drops a pending save when newer content arrives from disk", () => {
    auto.change("mine", () => "OLD CANVAS");
    auto.markSaved("merged");
    vi.advanceTimersByTime(2000);
    expect(saved).toEqual([]);
    auto.change("merged", () => "x");
    vi.advanceTimersByTime(2000);
    expect(saved).toEqual([]);
  });

  it("holds saves while the assistant works and saves the latest after", () => {
    auto.hold(true);
    auto.change("mine", () => "MINE");
    vi.advanceTimersByTime(5000);
    expect(saved).toEqual([]);
    auto.change("mine2", () => "MINE2");
    auto.hold(false);
    vi.advanceTimersByTime(800);
    expect(saved).toEqual(["MINE2"]);
  });

  it("flushes a pending save immediately, but not while held", () => {
    auto.change("a", () => "A");
    auto.flush();
    expect(saved).toEqual(["A"]);
    vi.advanceTimersByTime(2000);
    expect(saved).toEqual(["A"]);

    auto.hold(true);
    auto.change("b", () => "B");
    auto.flush();
    expect(saved).toEqual(["A"]);
  });
});
