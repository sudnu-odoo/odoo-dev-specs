import { describe, expect, it } from "vitest";
import { mergeElements } from "./merge";

const el = (id: string, version: number, note = "") => ({ id, version, note });

describe("mergeElements", () => {
  it("adds what the assistant created", () => {
    expect(mergeElements([el("a", 1)], [el("a", 1), el("b", 1)])).toEqual([el("a", 1), el("b", 1)]);
  });

  it("takes the assistant's newer change", () => {
    expect(mergeElements([el("a", 1, "old")], [el("a", 2, "new")])).toEqual([el("a", 2, "new")]);
  });

  it("keeps the user's newer, not yet saved change", () => {
    expect(mergeElements([el("a", 3, "mine")], [el("a", 2, "disk")])).toEqual([el("a", 3, "mine")]);
  });

  it("keeps elements the user added since the last save", () => {
    expect(mergeElements([el("a", 1), el("new", 1)], [el("a", 1), el("b", 1)])).toEqual([
      el("a", 1),
      el("b", 1),
      el("new", 1),
    ]);
  });

  it("prefers the disk on equal versions", () => {
    expect(mergeElements([el("a", 2, "canvas")], [el("a", 2, "disk")])).toEqual([el("a", 2, "disk")]);
  });
});
