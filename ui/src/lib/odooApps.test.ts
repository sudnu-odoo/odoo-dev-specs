import { describe, expect, it } from "vitest";
import { appsText, sizeText, usedText } from "./odooApps";

describe("database texts", () => {
  it("names apps the way people know them", () => {
    expect(appsText(["sale_management", "stock", "x_custom"])).toBe("Sales, Inventory, x_custom");
    expect(appsText([])).toBe("Base only");
  });

  it("shows sizes and last use in plain words", () => {
    expect(sizeText(5 * 1024 ** 2)).toBe("5 MB");
    expect(sizeText(3 * 1024 ** 3)).toBe("3.0 GB");
    const now = Date.UTC(2026, 10, 18, 12);
    expect(usedText(now - 3_600_000, now)).toBe("used today");
    expect(usedText(now - 3 * 86_400_000, now)).toBe("used 3 days ago");
    expect(usedText(0, now)).toBe("never used");
  });
});
