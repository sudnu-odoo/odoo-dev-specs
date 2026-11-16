import { restoreElements } from "@excalidraw/excalidraw";
import { fitContainers, type LayoutElement } from "./fit";

type Elements = Parameters<typeof restoreElements>[0];

/**
 * Lay labels out with the real font: Excalidraw re-wraps every label to its box
 * (the assistant's tools can only estimate text size), then boxes grow to fit.
 */
export function layoutElements(elements: Elements) {
  const measured = restoreElements(elements, null, { refreshDimensions: true, repairBindings: true });
  return fitContainers(measured as unknown as LayoutElement[]) as unknown as typeof measured;
}
