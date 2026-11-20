import { describe, expect, it } from "vitest";
import { newestFirst, preferredVersion } from "./versions";

describe("versions on this computer", () => {
  it("sorts newest first", () => {
    expect(newestFirst(["17.0", "saas-19.2", "19.0", "saas-18.4"])).toEqual([
      "saas-19.2",
      "19.0",
      "saas-18.4",
      "17.0",
    ]);
  });

  it("never requires 19.0: it starts with what is here", () => {
    expect(preferredVersion(["17.0", "18.0"])).toBe("18.0");
    expect(preferredVersion(["saas-18.4"])).toBe("saas-18.4");
    expect(preferredVersion(["18.0", "19.0"], "18.0")).toBe("18.0");
    expect(preferredVersion(["18.0"], "19.0")).toBe("18.0");
    expect(preferredVersion([])).toBeNull();
  });
});
