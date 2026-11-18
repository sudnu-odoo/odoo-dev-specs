// Cross-boundary test: the diagram written by the Rust `excalidraw` crate (the
// assistant's tools) must load in the real Excalidraw editor without losing anything.
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Tests run from ui/ (npm scripts always do); Rust writes the fixture under target/.
const FIXTURE = resolve(process.cwd(), "../target/test-fixtures/generated.excalidraw");

// jsdom has no canvas; Excalidraw only needs text measuring from it here.
HTMLCanvasElement.prototype.getContext = (() => ({
  measureText: (t: string) => ({
    width: t.length * 8,
    actualBoundingBoxAscent: 8,
    actualBoundingBoxDescent: 2,
  }),
  font: "",
  filter: "none",
})) as never;

type El = {
  id: string;
  type: string;
  isDeleted: boolean;
  containerId?: string | null;
  frameId?: string | null;
  fileId?: string;
  startBinding?: { elementId: string } | null;
  endBinding?: { elementId: string } | null;
  boundElements?: { id: string; type: string }[] | null;
};

describe.skipIf(!existsSync(FIXTURE))("diagrams written by the assistant's tools", () => {
  it("load in Excalidraw with every element, binding and image intact", async () => {
    const { restore } = await import("@excalidraw/excalidraw");
    const written = JSON.parse(readFileSync(FIXTURE, "utf8"));
    const restored = restore(written, null, null);
    const byId = new Map((restored.elements as unknown as El[]).map((e) => [e.id, e]));

    for (const original of written.elements as El[]) {
      const e = byId.get(original.id);
      expect(e, `${original.type} ${original.id} was dropped`).toBeDefined();
      expect(e!.type).toBe(original.type);
      expect(e!.isDeleted).toBe(original.isDeleted);
      expect(e!.frameId ?? null).toBe(original.frameId ?? null);
      expect(e!.containerId ?? null).toBe(original.containerId ?? null);
      expect(e!.startBinding?.elementId).toBe(original.startBinding?.elementId);
      expect(e!.endBinding?.elementId).toBe(original.endBinding?.elementId);
      expect(new Set((e!.boundElements ?? []).map((b) => b.id))).toEqual(
        new Set((original.boundElements ?? []).map((b) => b.id)),
      );
    }
    const kinds = new Set((written.elements as El[]).map((e) => e.type));
    expect(kinds).toEqual(new Set(["frame", "rectangle", "text", "arrow", "image"]));

    const image = (restored.elements as unknown as El[]).find((e) => e.type === "image")!;
    expect(restored.files[image.fileId!]?.dataURL).toMatch(/^data:image\/png;base64,/);
  }, 20_000); // Loading Excalidraw is slow when the whole suite runs at once.
});
