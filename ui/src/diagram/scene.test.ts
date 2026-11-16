import { describe, expect, it } from "vitest";
import { parseScene, sceneFingerprint } from "./scene";

describe("parseScene", () => {
  it("reads elements, files and the background", () => {
    const data = parseScene(
      JSON.stringify({
        type: "excalidraw",
        elements: [{ id: "a" }],
        appState: { viewBackgroundColor: "#fff", collaborators: {} },
        files: { f1: { id: "f1" } },
      }),
    );
    expect(data.elements).toEqual([{ id: "a" }]);
    expect(data.files).toEqual({ f1: { id: "f1" } });
    expect(data.appState).toEqual({ viewBackgroundColor: "#fff" });
  });

  it("falls back to an empty canvas for unreadable data", () => {
    for (const bad of ["", "nope", "null", '{"elements":"x"}']) {
      expect(parseScene(bad)).toEqual({ elements: [], files: {}, appState: {} });
    }
  });
});

describe("sceneFingerprint", () => {
  const els = [
    { id: "a", version: 1 },
    { id: "b", version: 4 },
  ];

  it("is stable for the same content", () => {
    expect(sceneFingerprint(els, {})).toBe(sceneFingerprint([...els], {}));
  });

  it("changes when an element changes, is added, or a file is added", () => {
    const base = sceneFingerprint(els, {});
    expect(sceneFingerprint([{ id: "a", version: 2 }, els[1]], {})).not.toBe(base);
    expect(sceneFingerprint([...els, { id: "c", version: 1 }], {})).not.toBe(base);
    expect(sceneFingerprint(els, { f: {} })).not.toBe(base);
  });
});
