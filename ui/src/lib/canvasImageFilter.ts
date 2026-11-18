/**
 * In its dark theme, Excalidraw inverts the whole canvas with a CSS filter and draws
 * each image with `ctx.filter = IMAGE_INVERT_FILTER`, so photos come out in their real
 * colours. WebKitGTK (the app's web view) has no `ctx.filter`: the images stayed
 * inverted. Where it is missing, drawImage gets the same filter applied by hand.
 */

/** The filter Excalidraw sets for images in dark mode (0.18). */
export const IMAGE_INVERT_FILTER = "invert(100%) hue-rotate(180deg) saturate(1.25)";

const clamp = (x: number) => (x < 0 ? 0 : x > 255 ? 255 : x);

/** One pixel through invert(100%), hue-rotate(180deg), saturate(1.25), per the CSS spec. */
export function darkImagePixel(r: number, g: number, b: number): [number, number, number] {
  const [ir, ig, ib] = [255 - r, 255 - g, 255 - b];
  const hr = clamp(-0.574 * ir + 1.43 * ig + 0.144 * ib);
  const hg = clamp(0.426 * ir + 0.43 * ig + 0.144 * ib);
  const hb = clamp(0.426 * ir + 1.43 * ig - 0.856 * ib);
  return [
    clamp(1.19675 * hr - 0.17875 * hg - 0.018 * hb),
    clamp(-0.05325 * hr + 1.07125 * hg - 0.018 * hb),
    clamp(-0.05325 * hr - 0.17875 * hg + 1.232 * hb),
  ];
}

type Source = CanvasImageSource & { width: number; height: number };
const filtered = new WeakMap<object, HTMLCanvasElement>();

function darkCopy(source: Source): HTMLCanvasElement | null {
  const cached = filtered.get(source);
  if (cached) return cached;
  const w = (source as HTMLImageElement).naturalWidth || source.width;
  const h = (source as HTMLImageElement).naturalHeight || source.height;
  if (!w || !h) return null;
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.drawImage(source, 0, 0);
  const data = ctx.getImageData(0, 0, w, h);
  const px = data.data;
  for (let i = 0; i < px.length; i += 4) {
    const [r, g, b] = darkImagePixel(px[i], px[i + 1], px[i + 2]);
    px[i] = r;
    px[i + 1] = g;
    px[i + 2] = b;
  }
  ctx.putImageData(data, 0, 0);
  filtered.set(source, canvas);
  return canvas;
}

type FilterCtx = Omit<CanvasRenderingContext2D, "filter"> & {
  filter?: string;
  __filters?: (string | undefined)[];
};

/** Patch the 2D context where `filter` is missing (pass the prototype; tests pass a fake). */
export function installImageFilterFallback(target: CanvasRenderingContext2D) {
  if ("filter" in target) return;
  const proto = target as unknown as FilterCtx;
  const { drawImage, save, restore } = proto;
  // Without native support, `filter` is a plain property: keep it in step with save/restore.
  proto.save = function (this: FilterCtx) {
    (this.__filters ??= []).push(this.filter);
    save.call(this);
  };
  proto.restore = function (this: FilterCtx) {
    this.filter = this.__filters?.pop();
    restore.call(this);
  };
  proto.drawImage = function (this: FilterCtx, source: CanvasImageSource, ...rest: number[]) {
    const copy = this.filter === IMAGE_INVERT_FILTER ? darkCopy(source as Source) : null;
    return (drawImage as (...a: unknown[]) => void).call(this, copy ?? source, ...rest);
  } as CanvasRenderingContext2D["drawImage"];
}

if (typeof CanvasRenderingContext2D !== "undefined") {
  installImageFilterFallback(CanvasRenderingContext2D.prototype);
}
