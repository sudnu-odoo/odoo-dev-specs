import type { ExcalidrawInitialDataState } from "@excalidraw/excalidraw/types";

type Elements = readonly { id: string; version: number }[];

const EMPTY: ExcalidrawInitialDataState = { elements: [], files: {}, appState: {} };

/** Initial canvas data from a stored .excalidraw document; empty when unreadable. */
export function parseScene(json: string): ExcalidrawInitialDataState {
  let doc: unknown;
  try {
    doc = JSON.parse(json);
  } catch {
    return EMPTY;
  }
  if (!doc || typeof doc !== "object" || !Array.isArray((doc as { elements?: unknown }).elements)) {
    return EMPTY;
  }
  const { elements, files, appState } = doc as {
    elements: ExcalidrawInitialDataState["elements"];
    files?: ExcalidrawInitialDataState["files"];
    appState?: { viewBackgroundColor?: string };
  };
  // Only the persistent part of appState; UI state like collaborators must not be restored.
  const background = appState?.viewBackgroundColor;
  return {
    elements,
    files: files ?? {},
    appState: background ? { viewBackgroundColor: background } : {},
  };
}

/** Changes whenever an element or an embedded file changes; used to skip no-op saves. */
export function sceneFingerprint(elements: Elements, files: Record<string, unknown>): string {
  const parts = elements.map((e) => `${e.id}:${e.version}`).join("|");
  return `${parts}#${Object.keys(files).sort().join(",")}`;
}
