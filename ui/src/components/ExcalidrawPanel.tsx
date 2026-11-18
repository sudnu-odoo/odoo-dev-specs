import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Excalidraw, exportToBlob, loadFromBlob, serializeAsJSON } from "@excalidraw/excalidraw";
import "@excalidraw/excalidraw/index.css";
import type { ExcalidrawImperativeAPI } from "@excalidraw/excalidraw/types";
import { layoutElements } from "../diagram/layout";
import { Autosave } from "../diagram/autosave";
import { layoutChanged } from "../diagram/fit";
import { mergeElements } from "../diagram/merge";
import { parseScene, sceneFingerprint } from "../diagram/scene";
import { saveFile } from "../lib/saveFile";
import { useResolvedCanvasTheme } from "../lib/canvasTheme";

/** Wait this long after the last edit before saving to disk. */
const AUTOSAVE_MS = 800;

export interface ExcalidrawPanelProps {
  /** Session title, also used as the download file name. */
  name: string;
  /** Stored .excalidraw JSON to start from. */
  scene: string;
  /** Called with the full .excalidraw JSON after the user changed the diagram. */
  onSceneChange: (scene: string) => void;
  /** Extra toolbar buttons. */
  actions?: ReactNode;
  /** A newer copy of the diagram from disk (e.g. edited by the assistant) to merge in. */
  incoming?: IncomingScene;
  /** Do not write to disk for now (the assistant is editing the same file). */
  holdSaves?: boolean;
}

export interface IncomingScene {
  scene: string;
  /** Increases with every reload, so the same text can be applied again. */
  revision: number;
}

function fileNameFor(title: string) {
  return title.replace(/[\\/:*?"<>|]+/g, " ").trim() || "spec";
}

export default function ExcalidrawPanel({
  name,
  scene,
  onSceneChange,
  actions,
  incoming,
  holdSaves,
}: ExcalidrawPanelProps) {
  const theme = useResolvedCanvasTheme();
  const [api, setApi] = useState<ExcalidrawImperativeAPI | null>(null);
  const [error, setError] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const fileName = fileNameFor(name);
  // The panel is remounted per session, so the initial scene is read once. Labels are
  // laid out before Excalidraw gets the scene: its API exists before it has loaded the
  // initial data, so changing the scene through the API at that point would race it.
  const [initialData, diskFingerprint] = useMemo(() => {
    const disk = parseScene(scene);
    const elements = disk.elements ?? [];
    return [{ ...disk, elements: layoutElements(elements) }, sceneFingerprint(elements, disk.files ?? {})];
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
  const saveRef = useRef(onSceneChange);
  saveRef.current = onSceneChange;
  const autosave = useMemo(
    () => new Autosave((json) => saveRef.current(json), AUTOSAVE_MS, diskFingerprint),
    [diskFingerprint],
  );
  // Leaving the session (or the window) saves a pending edit right away.
  useEffect(() => () => autosave.flush(), [autosave]);

  useEffect(() => autosave.hold(!!holdSaves), [autosave, holdSaves]);

  // Fonts load lazily: lay labels out again with the real font once they arrive.
  useEffect(() => {
    if (!api) return;
    const relayout = () => {
      const current = api.getSceneElementsIncludingDeleted();
      if (!current.some((e) => !e.isDeleted)) return; // not loaded yet: never write an empty scene
      const next = layoutElements(current);
      if (layoutChanged(current as never, next as never)) api.updateScene({ elements: next });
    };
    document.fonts?.ready.then(relayout);
    document.fonts?.addEventListener("loadingdone", relayout);
    return () => document.fonts?.removeEventListener("loadingdone", relayout);
  }, [api]);

  // Merge a newer copy from disk (the assistant's edits) into the canvas, keeping
  // the user's own newer edits. The merged canvas is saved only if it differs from disk.
  useEffect(() => {
    if (!api || !incoming) return;
    const disk = parseScene(incoming.scene);
    const diskElements = disk.elements ?? [];
    autosave.markSaved(sceneFingerprint(diskElements, disk.files ?? {}));
    api.addFiles(Object.values(disk.files ?? {}));
    api.updateScene({
      elements: layoutElements(mergeElements(api.getSceneElementsIncludingDeleted(), diskElements)),
    });
  }, [api, incoming?.revision]); // eslint-disable-line react-hooks/exhaustive-deps

  async function openFile(file: File) {
    if (!api) return;
    try {
      const scene = await loadFromBlob(file, null, null);
      api.updateScene({ elements: scene.elements, appState: { ...scene.appState } });
      api.addFiles(Object.values(scene.files));
      api.scrollToContent(undefined, { fitToContent: true });
      setError(null);
    } catch {
      setError(`${file.name} is not a valid .excalidraw file.`);
    }
  }

  async function trySave(blob: Blob, name: string, filterName: string, ext: string) {
    try {
      await saveFile(blob, name, filterName, ext);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  async function download() {
    if (!api) return;
    const json = serializeAsJSON(api.getSceneElements(), api.getAppState(), api.getFiles(), "local");
    await trySave(
      new Blob([json], { type: "application/json" }),
      `${fileName}.excalidraw`,
      "Excalidraw",
      "excalidraw",
    );
  }

  async function exportPng() {
    if (!api) return;
    const blob = await exportToBlob({
      elements: api.getSceneElements(),
      appState: { ...api.getAppState(), exportBackground: true },
      files: api.getFiles(),
      mimeType: "image/png",
    });
    await trySave(blob, `${fileName}.png`, "PNG image", "png");
  }

  return (
    <section className="artifact">
      <header className="artifact-toolbar">
        <span className="artifact-title">{fileName}.excalidraw</span>
        {error && <span className="artifact-error">{error}</span>}
        <div className="artifact-actions">
          {actions}
          <button
            className="btn"
            title="Replace this diagram with an .excalidraw file"
            onClick={() => fileInput.current?.click()}
          >
            Import
          </button>
          <button className="btn" onClick={exportPng}>
            Export PNG
          </button>
          <button className="btn btn-primary" onClick={download}>
            Download .excalidraw
          </button>
        </div>
        <input
          ref={fileInput}
          type="file"
          accept=".excalidraw,application/json"
          hidden
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) openFile(f);
            e.target.value = "";
          }}
        />
      </header>
      <div className="artifact-canvas">
        <Excalidraw
          excalidrawAPI={setApi}
          // Light by default, like paper and the PNG export; chosen in Settings. In dark,
          // Excalidraw counter-inverts images, so screenshots keep their real colours.
          theme={theme}
          initialData={initialData}
          onChange={(elements, appState, files) =>
            autosave.change(sceneFingerprint(elements, files), () =>
              serializeAsJSON(elements, appState, files, "local"),
            )
          }
        />
      </div>
    </section>
  );
}
