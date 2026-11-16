import { beforeAll, describe, expect, it } from "vitest";

// jsdom has no canvas: a fake font where every character is 8px wide.
beforeAll(() => {
  HTMLCanvasElement.prototype.getContext = (() => ({
    measureText: (t: string) => ({
      width: t.length * 8,
      actualBoundingBoxAscent: 8,
      actualBoundingBoxDescent: 2,
    }),
    font: "",
    filter: "none",
  })) as never;
});

const base = {
  angle: 0,
  strokeColor: "#000",
  backgroundColor: "transparent",
  fillStyle: "solid",
  strokeWidth: 2,
  strokeStyle: "solid",
  roughness: 1,
  opacity: 100,
  groupIds: [],
  frameId: null,
  seed: 1,
  versionNonce: 1,
  isDeleted: false,
  updated: 1,
  link: null,
  locked: false,
};

describe("layoutElements", () => {
  it("wraps an estimated label inside its box and grows the box to fit", async () => {
    const { layoutElements } = await import("./layout");
    const label = "Order lines: Product, Quantity, Unit price, Taxes, Subtotal and Total";
    const out = layoutElements([
      {
        ...base,
        id: "box",
        type: "rectangle",
        x: 0,
        y: 0,
        width: 200,
        height: 40,
        version: 1,
        roundness: null,
        boundElements: [{ type: "text", id: "t" }],
      },
      // As the Rust tools write it: one long line, sized by an estimate.
      {
        ...base,
        id: "t",
        type: "text",
        x: 0,
        y: 0,
        width: 600,
        height: 25,
        version: 1,
        roundness: null,
        boundElements: null,
        text: label,
        originalText: label,
        fontSize: 20,
        fontFamily: 5,
        textAlign: "center",
        verticalAlign: "middle",
        containerId: "box",
        autoResize: true,
        lineHeight: 1.25,
      },
    ] as never) as unknown as { id: string; width: number; height: number; x: number; text?: string }[];

    const box = out.find((e) => e.id === "box")!;
    const text = out.find((e) => e.id === "t")!;
    expect(text.text).toContain("\n");
    expect(text.width).toBeLessThanOrEqual(box.width);
    expect(box.height).toBeGreaterThanOrEqual(text.height);
  });
});
