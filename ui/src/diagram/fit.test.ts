import { describe, expect, it } from "vitest";
import { fitContainers, type LayoutElement } from "./fit";

const box = (over: Partial<LayoutElement> = {}): LayoutElement => ({
  id: "box",
  type: "rectangle",
  x: 0,
  y: 0,
  width: 200,
  height: 40,
  version: 1,
  ...over,
});
const label = (over: Partial<LayoutElement> = {}): LayoutElement => ({
  id: "label",
  type: "text",
  x: 10,
  y: 5,
  width: 180,
  height: 75,
  version: 1,
  containerId: "box",
  ...over,
});

describe("fitContainers", () => {
  it("grows a box that is shorter than its label and centres the label", () => {
    const [b, l] = fitContainers([box(), label()]);
    expect(b.height).toBeGreaterThanOrEqual(75 + 20);
    expect(b.version).toBe(2);
    expect(l.y + l.height / 2).toBeCloseTo(b.y + b.height / 2);
    expect(l.x + l.width / 2).toBeCloseTo(b.x + b.width / 2);
    expect(l.version).toBe(2);
  });

  it("leaves boxes that already fit untouched", () => {
    const input = [box({ height: 120 }), label({ y: 22 })];
    const out = fitContainers(input);
    expect(out[0]).toBe(input[0]);
    expect(out[1]).toBe(input[1]);
  });

  it("ignores arrow labels and deleted text", () => {
    const arrow = box({ id: "arrow", type: "arrow", height: 0 });
    const input = [arrow, label({ containerId: "arrow" }), box(), label({ isDeleted: true })];
    expect(fitContainers(input)).toEqual(input);
  });
});
