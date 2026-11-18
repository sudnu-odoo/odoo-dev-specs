import { describe, expect, it } from "vitest";
import { IMAGE_INVERT_FILTER, darkImagePixel, installImageFilterFallback } from "./canvasImageFilter";

/** What the dark canvas then does to every pixel: invert(93%) hue-rotate(180deg). */
function darkCanvas([r, g, b]: number[]) {
  const inv = (c: number) => c * (1 - 0.93) + (255 - c) * 0.93;
  const [ir, ig, ib] = [inv(r), inv(g), inv(b)];
  const k = (x: number) => Math.min(255, Math.max(0, x));
  return [
    k(-0.574 * ir + 1.43 * ig + 0.144 * ib),
    k(0.426 * ir + 0.43 * ig + 0.144 * ib),
    k(0.426 * ir + 1.43 * ig - 0.856 * ib),
  ];
}

describe("dark-mode image filter for WebKit", () => {
  it("draws images so the dark canvas shows them in about their real colours", () => {
    for (const colour of [
      [255, 255, 255], // Odoo's white page
      [113, 75, 103], // Odoo purple
      [220, 53, 69], // a red banner
      [33, 37, 41], // dark text
    ]) {
      const seen = darkCanvas(darkImagePixel(colour[0], colour[1], colour[2]));
      seen.forEach((c, i) => expect(Math.abs(c - colour[i]), `${colour} -> ${seen}`).toBeLessThan(40));
    }
  });

  it("uses Excalidraw's own filter value", () => {
    expect(IMAGE_INVERT_FILTER).toBe("invert(100%) hue-rotate(180deg) saturate(1.25)");
  });

  it("changes nothing where the browser supports canvas filters", () => {
    const proto = { drawImage() {}, save() {}, restore() {}, filter: "none" };
    const before = proto.drawImage;
    installImageFilterFallback(proto as unknown as CanvasRenderingContext2D);
    expect(proto.drawImage).toBe(before);
  });

  it("is installed where canvas filters are missing", () => {
    const proto = { drawImage() {}, save() {}, restore() {} };
    const before = proto.drawImage;
    installImageFilterFallback(proto as unknown as CanvasRenderingContext2D);
    expect(proto.drawImage).not.toBe(before);
  });
});
