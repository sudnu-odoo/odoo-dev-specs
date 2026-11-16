import { describe, expect, it } from "vitest";
import { titleFromMessage } from "./title";

describe("titleFromMessage", () => {
  it("uses the first line, with whitespace collapsed", () => {
    expect(titleFromMessage("  Add a   delivery window\non sale orders")).toBe("Add a delivery window");
  });

  it("cuts long text at a word with an ellipsis", () => {
    const title = titleFromMessage("word ".repeat(30))!;
    expect(title.length).toBeLessThanOrEqual(60);
    expect(title.endsWith("…")).toBe(true);
    expect(title).not.toContain(" …");
  });

  it("returns null for blank text", () => {
    expect(titleFromMessage(" \n ")).toBeNull();
  });
});
