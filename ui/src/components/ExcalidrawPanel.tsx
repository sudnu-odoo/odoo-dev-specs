import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Excalidraw, exportToBlob, loadFromBlob, serializeAsJSON } from "@excalidraw/excalidraw";
import "@excalidraw/excalidraw/index.css";
import type { ExcalidrawImperativeAPI } from "@excalidraw/excalidraw/types";
import { layoutElements } from "../diagram/layout";
import { mergeElements } from "../diagram/merge";
import { parseScene, sceneFingerprint } from "../diagram/scene";
import { saveFile } from "../lib/saveFile";

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
}

export interface IncomingScene {
  scene: string;
  /** Increases with every reload, so the same text can be applied again. */
  revision: number;
}

function useSystemTheme(): "light" | "dark" {
  const query = window.matchMedia("(prefers-color-scheme: dark)");
  const [dark, setDark] = useState(query.matches);
  useEffect(() => {
    const onChange = (e: MediaQueryListEvent) => setDark(e.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, [query]);
  return dark ? "dark" : "light";
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
}: ExcalidrawPanelProps) {
  const [api, setApi] = useState<ExcalidrawImperativeAPI | null>(null);
  const [error, setError] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const theme = useSystemTheme();
  const fileName = fileNameFor(name);
  // The panel is remounted per session, so the initial scene is read once.
  const initialData = useMemo(() => parseScene(scene), []); // eslint-disable-line react-hooks/exhaustive-deps
  const autosave = useAutosave(initialData, onSceneChange);

  // Labels are laid out with the real font once it is loaded (fonts load lazily, so
  // also after each later font load). Fixes diagrams whose sizes were only estimated.
  useEffect(() => {
    if (!api) return;
    const relayout = () =>
      api.updateScene({ elements: layoutElements(api.getSceneElementsIncludingDeleted()) });
    document.fonts?.ready.then(relayout);
    document.fonts?.addEventListener("loadingdone", relayout);
    return () => document.fonts?.removeEventListener("loadingdone", relayout);
  }, [api]);

  // Merge a newer copy from disk (the assistant's edits) into the canvas, keeping
  // the user's own newer edits. Saving the result only happens if it differs from disk.
  useEffect(() => {
    if (!api || !incoming) return;
    const disk = parseScene(incoming.scene);
    const diskElements = disk.elements ?? [];
    api.updateScene({
      elements: layoutElements(mergeElements(api.getSceneElementsIncludingDeleted(), diskElements)),
    });
    api.addFiles(Object.values(disk.files ?? {}));
    autosave.markSaved(sceneFingerprint(diskElements, disk.files ?? {}));
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
          theme={theme}
          initialData={initialData}
          onChange={autosave.onChange}
        />
      </div>
    </section>
  );
}

/**
 * Debounced save of real changes only (Excalidraw also fires onChange for
 * selection, scrolling and zoom). A pending save is flushed on unmount, so
 * switching session never loses the last edit.
 */
function useAutosave(initial: ReturnType<typeof parseScene>, save: (scene: string) => void) {
  const lastSaved = useRef(sceneFingerprint(initial.elements ?? [], initial.files ?? {}));
  const pending = useRef<{ timer: number; flush: () => void } | null>(null);
  const saveRef = useRef(save);
  saveRef.current = save;

  useEffect(
    () => () => {
      if (pending.current) {
        window.clearTimeout(pending.current.timer);
        pending.current.flush();
      }
    },
    [],
  );

  const onChange = (
    elements: Parameters<typeof serializeAsJSON>[0],
    appState: Parameters<typeof serializeAsJSON>[1],
    files: Parameters<typeof serializeAsJSON>[2],
  ) => {
    const fingerprint = sceneFingerprint(elements, files);
    if (fingerprint === lastSaved.current) return;
    if (pending.current) window.clearTimeout(pending.current.timer);
    const flush = () => {
      pending.current = null;
      lastSaved.current = fingerprint;
      saveRef.current(serializeAsJSON(elements, appState, files, "local"));
    };
    pending.current = { timer: window.setTimeout(flush, AUTOSAVE_MS), flush };
  };
  /** The disk already holds this content (e.g. it was just loaded from there). */
  const markSaved = (fingerprint: string) => {
    lastSaved.current = fingerprint;
  };
  return { onChange, markSaved };
}
