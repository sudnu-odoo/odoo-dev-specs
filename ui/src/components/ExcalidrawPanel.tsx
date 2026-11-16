import { useEffect, useRef, useState } from "react";
import { Excalidraw, exportToBlob, loadFromBlob, serializeAsJSON } from "@excalidraw/excalidraw";
import "@excalidraw/excalidraw/index.css";
import type { ExcalidrawImperativeAPI } from "@excalidraw/excalidraw/types";
import { saveFile } from "../lib/saveFile";

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

export default function ExcalidrawPanel() {
  const [api, setApi] = useState<ExcalidrawImperativeAPI | null>(null);
  const [fileName, setFileName] = useState("spec");
  const [error, setError] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const theme = useSystemTheme();

  async function openFile(file: File) {
    if (!api) return;
    try {
      const scene = await loadFromBlob(file, null, null);
      api.updateScene({ elements: scene.elements, appState: { ...scene.appState } });
      api.addFiles(Object.values(scene.files));
      api.scrollToContent(undefined, { fitToContent: true });
      setFileName(file.name.replace(/\.excalidraw$/i, ""));
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
          <button className="btn" onClick={() => fileInput.current?.click()}>
            Open
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
        <Excalidraw excalidrawAPI={setApi} theme={theme} />
      </div>
    </section>
  );
}
