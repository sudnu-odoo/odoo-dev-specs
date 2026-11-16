export type LayoutElement = {
  id: string;
  type: string;
  x: number;
  y: number;
  width: number;
  height: number;
  version: number;
  isDeleted?: boolean;
  containerId?: string | null;
};

/** Space kept between a label and the edge of its box. */
const PADDING = 10;

/** Grow boxes whose label is taller than the box, and re-centre those labels. */
export function fitContainers<T extends LayoutElement>(elements: readonly T[]): T[] {
  const byId = new Map(elements.map((e) => [e.id, e]));
  const grown = new Map<string, T>();
  for (const text of elements) {
    if (text.type !== "text" || text.isDeleted || !text.containerId) continue;
    const box = byId.get(text.containerId);
    if (!box || box.type === "arrow" || box.isDeleted) continue;
    const needed = text.height + 2 * PADDING;
    if (box.height >= needed) continue;
    const bigger = { ...box, height: needed, version: box.version + 1 };
    grown.set(box.id, bigger);
    grown.set(text.id, {
      ...text,
      x: bigger.x + (bigger.width - text.width) / 2,
      y: bigger.y + (needed - text.height) / 2,
      version: text.version + 1,
    });
  }
  return elements.map((e) => grown.get(e.id) ?? e);
}
